// Copyright © 2021 BigBlueButton Inc. and by respective authors
//
// This file is part of BigBlueButton open source conferencing system.
//
// BigBlueButton is free software: you can redistribute it and/or modify it
// under the terms of the GNU Lesser General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// BigBlueButton is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU Lesser General Public License for more
// details.
//
// You should have received a copy of the GNU Lesser General Public License
// along with BigBlueButton.  If not, see <http://www.gnu.org/licenses/>.

use libc::getresuid;
use regex::Regex;
use std::env;
use std::fmt;
use std::process::exit;
use std::process::Command;

#[macro_use]
extern crate lazy_static;

enum RecordingStage {
    PROCESS,
    PUBLISH,
}

impl fmt::Display for RecordingStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::PROCESS => "process",
                Self::PUBLISH => "publish",
            }
        )
    }
}

fn usage(arg0: &str) -> ! {
    eprintln!("Usage: {} process|publish RECORDING_ID", arg0);
    exit(1);
}

fn validate_bbb_id(id: &str) -> bool {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^[a-f0-9]{40}-[0-9]+$").unwrap();
    }
    RE.is_match(&id)
}

fn format_bbb_id(stage: RecordingStage, id: String) -> String {
    match stage {
        RecordingStage::PROCESS => id,
        RecordingStage::PUBLISH => format!("{}-capture", id),
    }
}

fn main() {
    // Validate user & permissions
    let mut ruid = 0;
    let mut euid = 0;
    let mut suid = 0;

    unsafe {
        getresuid(&mut ruid, &mut euid, &mut suid);
    }

    if euid != 0 {
        eprintln!("This application must be installed setuid root");
        exit(1);
    }
    if ruid == euid {
        eprintln!("Unable to determine real uid, please run as the bigbluebutton user");
        exit(1);
    }

    // Validate command line arguments
    let mut args = env::args();

    let arg0 = args
        .next()
        .unwrap_or_else(|| "bbb-playback-capture-wrapper".to_owned());

    let script = match &*args.next().unwrap_or_else(|| usage(&arg0)) {
        "process" => RecordingStage::PROCESS,
        "publish" => RecordingStage::PUBLISH,
        s => {
            eprintln!("Invalid recording stage: {}", s);
            exit(1);
        }
    };

    let recording_id = args.next().unwrap_or_else(|| usage(&arg0));
    if !validate_bbb_id(&recording_id) {
        eprintln!("Recording id is not correct format");
        exit(1);
    }

    // Run the recording script inside the docker environment
    let docker_status = Command::new("docker")
        .arg("run")
        // run options
        .arg("--rm")
        .arg("--user")
        .arg(format!("{}", ruid))
        .arg("--mount")
        .arg("type=bind,src=/var/bigbluebutton,dst=/var/bigbluebutton")
        .arg("--mount")
        .arg("type=bind,src=/var/log/bigbluebutton,dst=/var/log/bigbluebutton")
        // image
        .arg("bbb-playback-capture:latest")
        // command
        .arg(format!("{}/capture.rb", script))
        // command arguments
        .arg("-m")
        .arg(format_bbb_id(script, recording_id))
        // execution settings
        .env_clear()
        .current_dir("/")
        // run and return status
        .status();
    match docker_status {
        Ok(status) => {
            match status.code() {
                Some(code) => eprintln!("Docker exited with status code: {}", code),
                None => eprintln!("Docker terminated by signal"),
            }
            if !status.success() {
                exit(status.code().unwrap_or(1));
            }
        }
        Err(err) => {
            eprintln!("Failed to start Docker: {}", err);
            exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_bbb_id() {
        assert!(validate_bbb_id(
            "0a838768c250342c90eed02b34b6d66c97fde0c9-1588887004652"
        ));
        assert!(!validate_bbb_id(
            "8768c250342c90eed02b34b6d66c97fd-1588887004652"
        ));
        assert!(!validate_bbb_id("0a838768c250342c90eed02b34b6d66c97fde0c9"));
        assert!(!validate_bbb_id(
            "0a838768c250342c90eed02b34b6d66c97fde0c9-1588887004652/../passwd"
        ));
        assert!(!validate_bbb_id(
            "../0a838768c250342c90eed02b34b6d66c97fde0c9-1588887004652"
        ));
    }

    #[test]
    fn test_format_bbb_id() {
        assert_eq!(
            format_bbb_id(
                RecordingStage::PROCESS,
                "0a838768c250342c90eed02b34b6d66c97fde0c9-1588887004652".to_owned()
            ),
            "0a838768c250342c90eed02b34b6d66c97fde0c9-1588887004652".to_owned()
        );
        assert_eq!(
            format_bbb_id(
                RecordingStage::PUBLISH,
                "0a838768c250342c90eed02b34b6d66c97fde0c9-1588887004652".to_owned()
            ),
            "0a838768c250342c90eed02b34b6d66c97fde0c9-1588887004652-capture".to_owned()
        );
    }
}
