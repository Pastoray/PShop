//! AWS SDK bootstrap. Against LocalStack, set `AWS_ENDPOINT_URL` (the SDK
//! honours it for every service) plus dummy credentials. In real AWS leave it
//! unset and rely on IRSA / the default credential chain.

use aws_config::{meta::region::RegionProviderChain, BehaviorVersion, SdkConfig};

pub async fn sdk_config() -> SdkConfig {
    let region = RegionProviderChain::default_provider().or_else("us-east-1");
    aws_config::defaults(BehaviorVersion::latest())
        .region(region)
        .load()
        .await
}

/// LocalStack needs path-style S3 addressing (`host/bucket/key`).
pub fn endpoint_overridden() -> bool {
    std::env::var("AWS_ENDPOINT_URL").is_ok()
}
