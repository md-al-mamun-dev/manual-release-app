import requests
import time
import sys
import uuid

BASE_URL = "http://127.0.0.1:8080/api"

proj_name = f"test-project-{uuid.uuid4().hex[:8]}"

print(f"1. Creating a new project: {proj_name}")
resp = requests.post(f"{BASE_URL}/projects", json={
    "name": proj_name,
    "repositoryPath": "/tmp/test-repo-release"
})
if resp.status_code not in (200, 201):
    print("Failed to create project:", resp.text)
    sys.exit(1)

project = resp.json()
project_id = project["id"]
print(f"Project created with ID: {project_id}")

print("2. Starting inspection...")
resp = requests.post(f"{BASE_URL}/projects/{project_id}/inspect")
if resp.status_code not in (200, 201, 202):
    print("Failed to start inspection:", resp.status_code, resp.text)
    sys.exit(1)

inspection = resp.json()
inspection_id = inspection["id"]
print(f"Inspection started with ID: {inspection_id}")

print("3. Waiting for inspection to succeed...")
for i in range(15):
    time.sleep(1)
    resp = requests.get(f"{BASE_URL}/projects/{project_id}/inspections/latest")
    if resp.status_code == 200:
        latest = resp.json()
        if latest.get("status") == "SUCCEEDED":
            print("Inspection succeeded!")
            break
        elif latest.get("status") == "FAILED":
            print("Inspection failed:", latest.get("error_message"))
            sys.exit(1)
else:
    print("Inspection timed out")
    sys.exit(1)

print("4. Creating a release...")
resp = requests.post(f"{BASE_URL}/projects/{project_id}/releases", json={
    "version": "v0.1.0",
    "inspectionId": inspection_id
})

if resp.status_code not in (200, 201):
    print("Failed to create release:", resp.status_code, resp.text)
    sys.exit(1)

release = resp.json()
print("Release created successfully!")
print(release)
