export interface S3Connection {
  id: string;
  name: string;
  endpoint?: string;
  region: string;
  accessKeyId: string;
  forcePathStyle: boolean;
  skipTlsVerify: boolean;
  defaultBucket?: string;
}

export interface S3ConnectionInput {
  name: string;
  endpoint?: string;
  region: string;
  accessKeyId: string;
  secretAccessKey: string;
  /** STS session token — sent on save only; never returned from list/get. */
  sessionToken?: string;
  forcePathStyle: boolean;
  skipTlsVerify: boolean;
  defaultBucket?: string;
}

export interface ProviderPreset {
  id: string;
  label: string;
  endpoint?: string;
  region: string;
  forcePathStyle: boolean;
}

export const PROVIDER_PRESETS: ProviderPreset[] = [
  { id: "aws", label: "AWS S3", region: "us-east-1", forcePathStyle: false },
  {
    id: "minio",
    label: "MinIO",
    endpoint: "http://127.0.0.1:9000",
    region: "us-east-1",
    forcePathStyle: true,
  },
  {
    id: "r2",
    label: "Cloudflare R2",
    endpoint: "https://<account_id>.r2.cloudflarestorage.com",
    region: "auto",
    forcePathStyle: false,
  },
  {
    id: "do",
    label: "DigitalOcean Spaces",
    endpoint: "https://nyc3.digitaloceanspaces.com",
    region: "nyc3",
    forcePathStyle: false,
  },
  {
    id: "b2",
    label: "Backblaze B2",
    endpoint: "https://s3.us-west-004.backblazeb2.com",
    region: "us-west-004",
    forcePathStyle: true,
  },
  { id: "custom", label: "Custom", region: "us-east-1", forcePathStyle: true },
];
