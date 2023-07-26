// Copyright 2022-2023, Collabora, Ltd.
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::iter;

use xrpicker::platform::PlatformApiLayer;
use xrpicker::{make_platform, platform::PlatformRuntime, ActiveState, Platform};

fn main() {
    let platform = make_platform();
    let active_data = platform.get_active_runtime_data();
    let (runtimes, nonfatal_runtime_errors) = platform
        .find_available_runtimes(Box::new(iter::empty()))
        .unwrap();
    let (api_layers, nonfatal_api_layer_errors) = platform
        .find_available_api_layers(Box::new(iter::empty()))
        .unwrap();

    println!("\nRuntimes:");
    for runtime in runtimes {
        println!(
            "- {}: {:?} - {:?}",
            runtime.get_runtime_name(),
            platform.get_runtime_active_state(&runtime, &active_data),
            runtime
        );
    }

    println!("\nApi Layers:");
    for layer in api_layers {
        println!(
            "- {}: {:?} - {:?}",
            layer.get_layer_name(),
            layer
                .is_active()
                .unwrap_or(ActiveState::ActiveIndependentRuntime),
            layer
        );
    }

    if !nonfatal_runtime_errors.is_empty() {
        println!("\nNon-fatal runtime errors:");
        for e in nonfatal_runtime_errors {
            println!("- Manifest: {} - Error: {:?}", e.0.display(), e.1);
        }
    }

    if !nonfatal_api_layer_errors.is_empty() {
        println!("\nNon-fatal api layer errors:");
        for e in nonfatal_api_layer_errors {
            println!("- Manifest: {} - Error: {:?}", e.0.display(), e.1);
        }
    }

    println!("\nActive runtime manifest path(s):");
    for path in platform.get_active_runtime_manifests() {
        println!("- {}", path.display());
    }

    println!("\nActive api layer manifest path(s):");
    for path in platform.get_active_api_layer_manifests() {
        println!("- {}", path.display());
    }
}
