use crate::domain::release::ReleaseStatus;

pub fn can_transition(from: ReleaseStatus, to: ReleaseStatus) -> bool {
    use ReleaseStatus::*;

    matches!(
        (from, to),
        (Created, SourceValidated)
            | (Created, Failed)
            | (SourceValidated, CiRunning)
            | (SourceValidated, Failed)
            | (CiRunning, CiPassed)
            | (CiRunning, Failed)
            | (CiPassed, ImageBuilding)
            | (CiPassed, Failed)
            | (ImageBuilding, ImageBuilt)
            | (ImageBuilding, Failed)
            | (ImageBuilt, ImageTesting)
            | (ImageBuilt, Failed)
            | (ImageTesting, ImageTested)
            | (ImageTesting, Failed)
            | (ImageTested, SecurityScanning)
            | (ImageTested, Failed)
            | (SecurityScanning, ScanPassed)
            | (SecurityScanning, SecurityFailed)
            | (SecurityScanning, Failed)
            | (ScanPassed, ImageApproved)
            | (ScanPassed, Failed)
            | (ImageApproved, Publishing)
            | (ImageApproved, Failed)
            | (Publishing, Published)
            | (Publishing, PublishFailed)
            | (Publishing, Failed)
            | (Published, StagingDeploying)
            | (Published, Failed)
            | (StagingDeploying, StagingVerified)
            | (StagingDeploying, Failed)
            | (StagingDeploying, RollingBack)
            | (StagingVerified, ProductionApproved)
            | (ProductionApproved, ProductionDeploying)
            | (ProductionApproved, Failed)
            | (ProductionDeploying, ProductionVerified)
            | (ProductionDeploying, RollingBack)
            | (ProductionDeploying, Failed)
            | (Failed, RollingBack)
            | (PublishFailed, RollingBack)
            | (RollingBack, RolledBack)
            | (RollingBack, RollbackFailed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::release::ReleaseStatus::*;

    #[test]
    fn allows_normal_ci_progression() {
        assert!(can_transition(Created, SourceValidated));

        assert!(can_transition(SourceValidated, CiRunning));

        assert!(can_transition(CiRunning, CiPassed));
    }

    #[test]
    fn requires_staging_before_production() {
        assert!(!can_transition(Published, ProductionDeploying));

        assert!(can_transition(StagingVerified, ProductionApproved));

        assert!(can_transition(ProductionApproved, ProductionDeploying));
    }

    #[test]
    fn prevents_arbitrary_state_skips() {
        assert!(!can_transition(Created, ProductionDeploying));

        assert!(!can_transition(CiRunning, ProductionVerified));

        assert!(!can_transition(ImageBuilt, Published));
    }

    #[test]
    fn allows_security_scanning_progression() {
        assert!(can_transition(ImageTested, SecurityScanning));
        assert!(can_transition(SecurityScanning, ScanPassed));
        assert!(can_transition(SecurityScanning, SecurityFailed));
        assert!(can_transition(ScanPassed, ImageApproved));
        assert!(can_transition(ImageApproved, Publishing));
        assert!(can_transition(Publishing, Published));
    }

    #[test]
    fn prevents_security_scanning_skips() {
        assert!(!can_transition(ImageTested, ImageApproved));
        assert!(!can_transition(ImageTested, Published));
        assert!(!can_transition(ImageTested, ScanPassed));
        assert!(!can_transition(SecurityScanning, Publishing));
        assert!(!can_transition(SecurityScanning, Published));
        assert!(!can_transition(SecurityFailed, ImageApproved));
        assert!(!can_transition(SecurityFailed, Published));
        assert!(!can_transition(ScanPassed, Published));
        assert!(!can_transition(ScanPassed, Publishing));
    }

    #[test]
    fn supports_rollback_flow() {
        assert!(can_transition(ProductionDeploying, RollingBack));

        assert!(can_transition(RollingBack, RolledBack));

        assert!(can_transition(RollingBack, RollbackFailed));
    }
}
