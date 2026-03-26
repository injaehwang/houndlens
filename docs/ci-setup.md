# CI/CD Setup Guide

omnilens works with **any CI platform** and **any git hosting**. No vendor lock-in.

## Quick start (any platform)

```bash
# One-line setup for any CI
curl -fsSL https://raw.githubusercontent.com/injaehwang/omnilens/main/ci/generic.sh | bash
```

Or in your CI config:
```bash
cargo install omnilens
omnilens ci
```

The `omnilens ci` command auto-detects your platform (GitHub, GitLab, or local) and adjusts behavior accordingly.

## Local development (no CI needed)

### Git hooks (recommended)

```bash
omnilens hook install
```

This installs:
- **pre-commit**: Runs `omnilens verify` on staged changes before each commit
- **pre-push**: Checks for breaking changes before pushing

```bash
omnilens hook status    # Check what's installed
omnilens hook uninstall # Remove hooks (restores backups)
```

Bypass when needed: `git commit --no-verify`

### Manual verification

```bash
# Verify working directory changes
omnilens verify

# Verify against last commit
omnilens verify --diff HEAD~1

# Verify against main branch
omnilens verify --diff main

# Verify specific files
omnilens verify --files src/auth.rs --files src/db.rs
```

## GitHub Actions

```yaml
# .github/workflows/omnilens.yml
on: [pull_request]
jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with: { fetch-depth: 0 }
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install omnilens
      - run: omnilens ci
```

Or use the action directly:
```yaml
      - uses: injaehwang/omnilens@v1
        with:
          comment: "true"
          fail-on: "error"
```

## GitLab CI

```yaml
# .gitlab-ci.yml
include:
  - remote: 'https://raw.githubusercontent.com/injaehwang/omnilens/main/ci/gitlab-ci.yml'
```

Or manually:
```yaml
omnilens:
  stage: test
  image: rust:latest
  script:
    - cargo install omnilens
    - omnilens ci --platform gitlab
  artifacts:
    reports:
      dotenv: omnilens.env
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
```

Set `GITLAB_TOKEN` in CI variables for MR comments.

## Jenkins

```groovy
pipeline {
    agent any
    stages {
        stage('Verify') {
            steps {
                sh 'cargo install omnilens'
                sh 'omnilens ci --fail-on error'
            }
            post {
                always {
                    archiveArtifacts artifacts: 'omnilens-report.json', allowEmptyArchive: true
                }
            }
        }
    }
}
```

## Bitbucket Pipelines

```yaml
pipelines:
  pull-requests:
    '**':
      - step:
          name: omnilens verify
          image: rust:latest
          script:
            - cargo install omnilens
            - omnilens ci --fail-on error
          artifacts:
            - omnilens-report.json
```

## Azure DevOps

```yaml
trigger:
  branches:
    include: [main]
pr:
  branches:
    include: [main]

pool:
  vmImage: 'ubuntu-latest'

steps:
  - script: |
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
      source $HOME/.cargo/env
      cargo install omnilens
      omnilens ci --fail-on error
    displayName: 'omnilens verify'
```

## CircleCI

```yaml
version: 2.1
jobs:
  verify:
    docker:
      - image: rust:latest
    steps:
      - checkout
      - run: cargo install omnilens
      - run: omnilens ci --fail-on error
      - store_artifacts:
          path: omnilens-report.json

workflows:
  verify:
    jobs:
      - verify
```

## Environment variables

`omnilens ci` reads these standard CI variables automatically:

| Variable | Platform | Purpose |
|----------|----------|---------|
| `GITHUB_ACTIONS` | GitHub | Platform detection |
| `GITHUB_BASE_REF` | GitHub | PR base branch |
| `GITHUB_OUTPUT` | GitHub | Step outputs |
| `GITHUB_STEP_SUMMARY` | GitHub | Job summary |
| `GITLAB_CI` | GitLab | Platform detection |
| `CI_MERGE_REQUEST_DIFF_BASE_SHA` | GitLab | MR base commit |
| `CI_MERGE_REQUEST_IID` | GitLab | MR number |
| `CHANGE_TARGET` | Jenkins | PR target branch |
| `BITBUCKET_PR_DESTINATION_BRANCH` | Bitbucket | PR target |
| `SYSTEM_PULLREQUEST_TARGETBRANCH` | Azure DevOps | PR target |

## Custom configuration

Override defaults with environment variables:

```bash
OMNILENS_DIFF_BASE=main omnilens ci          # Compare against main
OMNILENS_FAIL_ON=warning omnilens ci         # Stricter threshold
OMNILENS_FORMAT=json omnilens ci             # JSON output
```

Or CLI flags:

```bash
omnilens ci --platform gitlab --fail-on warning
omnilens --format sarif ci                    # SARIF for code scanning
```

## Output formats

| Format | Use case | Flag |
|--------|----------|------|
| `text` | Human reading, terminal | `--format text` (default) |
| `json` | Programmatic parsing, AI agents | `--format json` |
| `sarif` | GitHub/GitLab Code Scanning | `--format sarif` |
