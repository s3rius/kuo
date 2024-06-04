use std::time::Duration;

use clap::Parser;
use lettre::transport::smtp::authentication::Credentials;

use super::args::OperatorArgs;

#[derive(Clone)]
pub struct OperatorCtx {
    pub client: kube::Client,
    pub args: OperatorArgs,
    pub smtp: lettre::AsyncSmtpTransport<lettre::Tokio1Executor>,
}

impl OperatorCtx {
    pub async fn new() -> anyhow::Result<Self> {
        let args = OperatorArgs::parse();
        tracing::info!("Connecting to Kubernetes");
        let client = kube::Client::try_default().await?;
        tracing::info!("Connected to Kubernetes");
        let smtp: lettre::AsyncSmtpTransport<lettre::Tokio1Executor> =
            lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::from_url(&args.smtp_url)?
                .port(args.smtp_port)
                .credentials(Credentials::new(
                    args.smtp_user.clone(),
                    args.smtp_pass.clone(),
                ))
                .pool_config(
                    lettre::transport::smtp::PoolConfig::new()
                        .max_size(3)
                        .idle_timeout(Duration::from_secs(30)),
                )
                .build();
        tracing::info!("Testing SMTP connection");
        smtp.test_connection().await?;
        tracing::info!("SMTP connection successful");
        Ok(Self { args, client, smtp })
    }
}
