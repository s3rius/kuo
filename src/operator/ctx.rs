use std::time::Duration;

use clap::Parser;
use lettre::transport::smtp::authentication::Credentials;

use super::{args::OperatorArgs, error::KuoResult};

#[derive(Clone)]
pub struct OperatorCtx {
    pub client: kube::Client,
    pub args: OperatorArgs,
    pub smtp: Option<lettre::AsyncSmtpTransport<lettre::Tokio1Executor>>,
}

impl OperatorCtx {
    async fn get_smtp_transport(
        args: &OperatorArgs,
    ) -> KuoResult<Option<lettre::AsyncSmtpTransport<lettre::Tokio1Executor>>> {
        let Some(smtp_args) = &args.smtp_args else {
            tracing::info!("No SMTP configuration found. Skipping SMTP setup.");
            return Ok(None);
        };
        tracing::info!("Found SMTP configuration. Creating SMTP transport.");
        let smtp: lettre::AsyncSmtpTransport<lettre::Tokio1Executor> =
            lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::from_url(&smtp_args.url)?
                .port(smtp_args.port)
                .credentials(Credentials::new(
                    smtp_args.user.clone(),
                    smtp_args.password.clone(),
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
        return Ok(Some(smtp));
    }

    pub async fn new() -> KuoResult<Self> {
        let args = OperatorArgs::parse();
        tracing::info!("Connecting to Kubernetes");
        let client = kube::Client::try_default().await?;
        tracing::info!("Connected to Kubernetes");
        let smtp = Self::get_smtp_transport(&args).await?;
        Ok(Self { args, client, smtp })
    }
}
