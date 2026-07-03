import type { S3Connection } from "@/types/connection";

export const connA: S3Connection = {
  id: "conn-a",
  name: "Connection A",
  region: "us-east-1",
  accessKeyId: "key-a",
  forcePathStyle: false,
  skipTlsVerify: false,
  defaultBucket: "bucket-a",
};

export const connB: S3Connection = {
  id: "conn-b",
  name: "Connection B",
  region: "us-east-1",
  accessKeyId: "key-b",
  forcePathStyle: false,
  skipTlsVerify: false,
  defaultBucket: "bucket-b",
};
