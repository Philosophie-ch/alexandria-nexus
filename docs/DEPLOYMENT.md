# Deployment

Production runs one image tag from `ghcr.io/philosophie-ch/alexandria-nexus`. Two workflows are involved:

- `.github/workflows/release.yml`: on a published GitHub release, builds and pushes the image, then calls `deploy.yml`.
- `.github/workflows/deploy.yml`: deploys an image tag that already exists in ghcr.io. It runs from a release, or by hand.

Deploys are serialized (job-level `production-deploy` concurrency group) and never cancelled midway.

## Release

Publish a GitHub release tagged `vX.Y.Z`. The image is built, pushed, and deployed.

## Deploy an existing version

No build, no release. Use it to roll back, or to apply a change to how the container runs (flags, env, log options) using an image that is already built.

```bash
gh workflow run deploy.yml -f version=0.2.10
```

Or: GitHub, Actions, Deploy, Run workflow, enter the version. A leading `v` is accepted (`v0.2.10`). A tag that is not in ghcr.io fails at the pull, before the running container is touched.

Each deploy ends with the running image tag and digest, printed in the log and in the job summary. A failed health check fails the job and dumps the last 20 container log lines.

## Rollback

Rolling back is one deploy of the previous version (above). Check migrations first.

Migrations run automatically on startup and only go forward. Rolling back to a version older than the last applied migration runs old code against a newer schema.

Before rolling back, compare `migrations/` between the two versions:

```bash
git diff --stat vOLD..vNEW -- migrations/
```

- No migration in between: the rollback is safe.
- A migration in between: the rollback is unsafe without a database restore. Backups are in `~/shared/alexandria-backup/` on philo-000, made daily at 01:00.

## Registry credentials

The deploy logs in to ghcr.io with the job's `github.token` inside a temporary Docker config that is deleted on exit. Nothing is written to the deploy user's `~/.docker/config.json`.

## Manual runs

`workflow_dispatch` only appears under Run workflow once `deploy.yml` is on the default branch.
