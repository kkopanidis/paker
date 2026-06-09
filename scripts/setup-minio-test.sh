#!/usr/bin/env bash
# Prepare a local or CI MinIO instance for Paker S3 integration tests.
set -euo pipefail

ENDPOINT="${PAKER_TEST_S3_ENDPOINT:-${MINIO_ENDPOINT:-http://127.0.0.1:9000}}"
BUCKET="${PAKER_TEST_S3_BUCKET:-paker-test}"
ACCESS_KEY="${MINIO_ROOT_USER:-minioadmin}"
SECRET_KEY="${MINIO_ROOT_PASSWORD:-minioadmin}"

echo "Waiting for MinIO at ${ENDPOINT}..."
for _ in $(seq 1 30); do
  if curl -sf "${ENDPOINT}/minio/health/live" >/dev/null; then
    echo "MinIO is healthy."
    break
  fi
  sleep 1
done

if ! curl -sf "${ENDPOINT}/minio/health/live" >/dev/null; then
  echo "MinIO is not reachable at ${ENDPOINT}" >&2
  exit 1
fi

create_bucket() {
  if command -v aws >/dev/null 2>&1; then
    AWS_ACCESS_KEY_ID="$ACCESS_KEY" AWS_SECRET_ACCESS_KEY="$SECRET_KEY" \
      aws --endpoint-url "$ENDPOINT" s3 mb "s3://${BUCKET}" 2>/dev/null || true
    return
  fi

  if command -v docker >/dev/null 2>&1; then
    docker run --rm --add-host=host.docker.internal:host-gateway \
      minio/mc:latest sh -c "
        mc alias set paker '${ENDPOINT}' '${ACCESS_KEY}' '${SECRET_KEY}' &&
        mc mb 'paker/${BUCKET}' --ignore-existing
      "
    return
  fi

  echo "Install aws CLI or docker to create bucket ${BUCKET}" >&2
  exit 1
}

echo "Ensuring bucket s3://${BUCKET} exists..."
create_bucket
echo "MinIO test bucket ready: s3://${BUCKET} @ ${ENDPOINT}"
