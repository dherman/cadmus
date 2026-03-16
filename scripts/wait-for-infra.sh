#!/usr/bin/env bash
# Wait for Postgres and LocalStack (with S3 bucket) to be ready.
# Used by dev:server to avoid starting before infrastructure is up.
set -e

echo "Waiting for Postgres on port 5433..."
until nc -z localhost 5433 2>/dev/null; do
  sleep 1
done
echo "Postgres is accepting connections."

echo "Waiting for S3 bucket..."
until aws --endpoint-url=http://localhost:4566 --region us-east-1 s3 ls s3://cadmus-documents >/dev/null 2>&1; do
  sleep 1
done
echo "S3 bucket is ready."
