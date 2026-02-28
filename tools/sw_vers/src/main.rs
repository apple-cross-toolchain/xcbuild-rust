use anyhow::{bail, Result};
use clap::Parser;
use xcbuild_sdk::{find_developer_root, Configuration, Manager, Product};

#[derive(Parser)]
#[command(about = "Print macOS version information")]
#[group(multiple = false)]
struct Cli {
    #[arg(long = "productName", alias = "ProductName")]
    product_name: bool,

    #[arg(long = "productVersion", alias = "ProductVersion")]
    product_version: bool,

    #[arg(long = "productVersionExtra", alias = "ProductVersionExtra")]
    product_version_extra: bool,

    #[arg(long = "buildVersion", alias = "BuildVersion")]
    build_version: bool,
}

fn find_product() -> Result<Product> {
    let dev_root = find_developer_root().unwrap_or_default();
    let config = Configuration::load(&Configuration::default_paths());
    let manager = Manager::open(&dev_root, config.as_ref());

    if let Some(mgr) = &manager {
        for platform in &mgr.platforms {
            for target in &platform.targets {
                if let Some(product) = &target.product {
                    return Ok(product.clone());
                }
            }
        }
    }

    // Fall back to MACOSX_DEPLOYMENT_TARGET
    if let Ok(version) = std::env::var("MACOSX_DEPLOYMENT_TARGET") {
        return Ok(Product {
            name: Some("macOS".to_string()),
            version: Some(version),
            build_version: Some("0CFFFF".to_string()),
            ..Default::default()
        });
    }

    bail!("unable to determine macOS version")
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let product = find_product()?;

    if cli.product_name {
        println!("{}", product.name.as_deref().unwrap_or("macOS"));
    } else if cli.product_version {
        println!("{}", product.version.as_deref().unwrap_or(""));
    } else if cli.product_version_extra {
        // Rapid Security Response version; empty if none installed
        println!("{}", product.user_visible_version.as_deref().unwrap_or(""));
    } else if cli.build_version {
        println!("{}", product.build_version.as_deref().unwrap_or(""));
    } else {
        println!("ProductName:\t\t{}", product.name.as_deref().unwrap_or("macOS"));
        println!("ProductVersion:\t\t{}", product.version.as_deref().unwrap_or(""));
        if let Some(extra) = &product.user_visible_version {
            println!("ProductVersionExtra:\t{}", extra);
        }
        println!("BuildVersion:\t\t{}", product.build_version.as_deref().unwrap_or(""));
    }

    Ok(())
}
