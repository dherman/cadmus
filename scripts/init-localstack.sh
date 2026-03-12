#!/usr/bin/env bash
# Wait for LocalStack to be ready, then create the S3 bucket
set -e
echo "Waiting for LocalStack..."
until aws --endpoint-url=http://localhost:4566 --region us-east-1 s3 ls 2>/dev/null; do
  sleep 1
done
aws --endpoint-url=http://localhost:4566 --region us-east-1 s3 mb s3://cadmus-documents 2>/dev/null || true
echo "LocalStack ready, bucket created."
