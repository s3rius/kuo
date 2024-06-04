use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
};

use clap::Parser;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::CustomResourceExt;
use serde::Serialize;

#[cfg(unix)]
extern crate libc;

#[derive(Parser, Debug, Clone)]
struct CrdsArgs {
    out_file: Option<String>,
}

fn generate_crds_def(crds: Vec<CustomResourceDefinition>) -> anyhow::Result<String> {
    let mut serializer = serde_yaml::Serializer::new(Vec::new());
    for crd in crds {
        eprintln!("- Adding {}/{}", crd.spec.group, crd.spec.names.kind);
        crd.serialize(&mut serializer)?;
    }
    let serialized = serializer.into_inner()?;
    String::from_utf8(serialized).map_err(Into::into)
}
pub fn main() -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        unsafe {
            // This thing is pretty safe, actually.
            // We're just setting the SIGPIPE signal to be ignored.
            // This is because by default, Rust will panic if the process
            // tries to write to a pipe that has been closed.
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
        }
    }
    dotenvy::dotenv().ok();
    let args = CrdsArgs::parse();
    let defs = generate_crds_def(vec![kuo::crds::managed_user::ManagedUser::crd()])?;
    if let Some(out_file) = args.out_file {
        let output = OpenOptions::new()
            .write(true)
            .append(false)
            .create(true)
            .open(out_file)?;
        let mut writer = BufWriter::new(output);
        writer.write_all(defs.as_bytes())?;
    } else {
        println!("{}", defs);
    }
    Ok(())
}
