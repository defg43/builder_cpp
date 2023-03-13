use std::{fs::File, io::Read, path::Path, process::Command};
use toml::{Table, Value};
use colored::Colorize;

//Log utils
#[derive(PartialEq, PartialOrd, Debug)]
pub enum LogLevel {
    Debug,
    Info,
    Log,
    Warn,
    Error,
}

pub fn log(level: LogLevel, message: &str) {
    let level_str = match level {
        LogLevel::Debug => "[DEBUG]".purple(),
        LogLevel::Info => "[INFO]".blue(),
        LogLevel::Log => "[LOG]".green(),
        LogLevel::Warn => "[WARN]".yellow(),
        LogLevel::Error => "[ERROR]".red(),
    };
    let log_level = match std::env::var("BUILDER_CPP_LOG_LEVEL") {
        Ok(val) => {
            if val == "Debug" {
                LogLevel::Debug
            } else if val == "Info" {
                LogLevel::Info
            } else if val == "Log" {
                LogLevel::Log
            } else if val == "Warn" {
                LogLevel::Warn
            } else if val == "Error" {
                LogLevel::Error
            } else {
                LogLevel::Log
            }
        }
        Err(_) => LogLevel::Log,
    };
    if log_level == LogLevel::Debug {
        if level == LogLevel::Debug || level == LogLevel::Warn || level == LogLevel::Error {
            println!("{} {}", level_str, message);
        }
    } else if level >= log_level {
        println!("{} {}", level_str, message);
    }
}

//Toml utils
#[derive(Debug)]
pub struct BuildConfig {
    pub compiler: String,
    pub build_dir: String,
    pub obj_dir: String,
    pub packages: Vec<String>,
}

#[derive(Debug)]
pub struct TargetConfig {
    pub name: String,
    pub src: String,
    pub include_dir: String,
    pub typ: String,
    pub cflags: String,
    pub libs: String,
    pub deps: Vec<String>,
}

pub fn parse_config(path: &str) -> (BuildConfig, Vec<TargetConfig>) {
    //open toml file and parse it into a string
    let mut file = File::open(path).unwrap_or_else(|_| {
        log(LogLevel::Error, &format!("Could not open config file: {}", path));
        std::process::exit(1);
    });
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap_or_else(|_| {
        log(LogLevel::Error, &format!("Could not read config file: {}", path));
        std::process::exit(1);
    });
    let config = contents.parse::<Table>().unwrap_or_else(|e| {
        log(LogLevel::Error, &format!("Could not parse config file: {}", path));
        log(LogLevel::Error, &format!("Error: {}", e));
        std::process::exit(1);
    });


    let mut pkgs: Vec<String> = Vec::new();
    let empty_value = Value::Array(Vec::new());
    //pkgs is optional
    let pkgs_toml = config["build"].as_table().unwrap_or_else(|| {
        log(LogLevel::Error, "Could not find build in config file");
        std::process::exit(1);})
        .get("packages").unwrap_or_else(|| &empty_value)
        .as_array()
        .unwrap_or_else(|| {
            log(LogLevel::Error, "packages is not an array");
            std::process::exit(1);
        });

    for pkg in pkgs_toml            {
        pkgs.push(pkg.as_str().unwrap_or_else(|| {
            log(LogLevel::Error, "packages are a vec of strings");
            std::process::exit(1);
        }).to_string());
    }

    //parse the string into a struct
    let build_config = BuildConfig {
        compiler: config["build"]["compiler"].as_str().unwrap_or_else(|| {
            log(LogLevel::Error, "Could not find compiler in config file");
            std::process::exit(1);
        }).to_string(),
        build_dir: config["build"]["build_dir"].as_str().unwrap_or_else(|| {
            log(LogLevel::Error, "Could not find build_dir in config file");
            std::process::exit(1);
        }).to_string(),
        obj_dir: config["build"]["obj_dir"].as_str().unwrap_or_else(|| {
            log(LogLevel::Error, "Could not find obj_dir in config file");
            std::process::exit(1);
        }).to_string(),
        packages: pkgs,
    };

    let mut tgt = Vec::new();
    let targets = config["targets"].as_array().unwrap_or_else(|| {
        log(LogLevel::Error, "Could not find targets in config file");
        std::process::exit(1);
    });

    for target in targets {
        let mut deps: Vec<String> = Vec::new();
        let empty_value = Value::Array(Vec::new());
        //deps is optional
        let deps_toml = target.get("deps").unwrap_or_else(|| { &empty_value })
            .as_array()
            .unwrap_or_else(|| {
                log(LogLevel::Error, "Deps is not an array");
                std::process::exit(1);
            })
        ;
        for dep in deps_toml            {
            deps.push(dep.as_str().unwrap_or_else(|| {
                log(LogLevel::Error, "Deps are a vec of strings");
                std::process::exit(1);
            }).to_string());
        }

        let target_config = TargetConfig {
            name: target["name"].as_str().unwrap_or_else(|| {
                log(LogLevel::Error, "Could not find name in confiig file");
                std::process::exit(1);
            }).to_string(),
            src: target["src"].as_str().unwrap_or_else(|| {
                log(LogLevel::Error, "Could not find src in confiig file");
                std::process::exit(1);
            }).to_string(),
            include_dir: target["include_dir"].as_str().unwrap_or_else(|| {
                log(LogLevel::Error, "Could not find include_dir in confiig file");
                std::process::exit(1);
            }).to_string(),
            typ: target["type"].as_str().unwrap_or_else(|| {
                log(LogLevel::Error, "Could not find type in confiig file");
                std::process::exit(1);
            }).to_string(),
            cflags: target["cflags"].as_str().unwrap_or_else(|| {
                log(LogLevel::Error, "Could not find cflags in confiig file");
                std::process::exit(1);
            }).to_string(),
            libs: target["libs"].as_str().unwrap_or_else(|| {
                log(LogLevel::Error, "Could not find libs in confiig file");
                std::process::exit(1);
            }).to_string(),
            deps,
        };
        if target_config.typ != "exe" && target_config.typ != "dll" {
            log(LogLevel::Error, "Type must be exe or dll");
            std::process::exit(1);
        }
        tgt.push(target_config);
    }

    (build_config, tgt)
}

#[derive(Debug)]
pub struct Package {
    pub name: String,
    pub repo: String,
    pub branch: String,
    pub build_config: BuildConfig,
    pub target_configs: Vec<TargetConfig>,
}

impl Package {
    pub fn new(name: String, repo: String, branch: String, build_config: BuildConfig, target_configs: Vec<TargetConfig>) -> Package {
        Package {
            name,
            repo,
            branch,
            build_config,
            target_configs,
        }
    }
    pub fn parse_packages(path: &str) -> Vec<Package> {
        let mut packages: Vec<Package> = Vec::new();
        //initialize fields
        let mut name = String::new();
        let mut repo = String::new();
        let mut branch = String::new();
        let mut build_config = BuildConfig {
            compiler: String::new(),
            build_dir: String::new(),
            obj_dir: String::new(),
            packages: Vec::new(),
        };
        let mut target_configs = Vec::new();

        //parse the root toml file
        let (build_config_toml, _) = parse_config(path);
        for package in build_config_toml.packages {
            let deets = package.split_whitespace().collect::<Vec<&str>>();
            if deets.len() != 2 {
                log(LogLevel::Error, "Packages must be in the form of \"<git_repo> <branch>\"");
                std::process::exit(1);
            }
            repo = deets[0].to_string().replace(",", "");
            branch = deets[1].to_string();

            name = repo.split("/").collect::<Vec<&str>>()[1].to_string();
            let source_dir = format!("./.bld_cpp/sources/{}/", name);
            if !Path::new(&source_dir).exists() {
                Command::new("mkdir")
                    .arg("-p")
                    .arg(&source_dir)
                    .output()
                    .expect("Failed to execute mkdir");
                if !Path::new(&source_dir).exists() {
                    log(LogLevel::Error, &format!("Failed to create {}", source_dir));
                    std::process::exit(1);
                } else {
                    log(LogLevel::Info, &format!("Created {}", source_dir));
                }
                log(LogLevel::Log, &format!("Cloning {} into {}", repo, source_dir));
                let repo_https = format!("https://github.com/{}", repo);
                let mut cmd = Command::new("git");
                cmd.arg("clone")
                    .arg("--branch")
                    .arg(&branch)
                    .arg(&repo_https)
                    .arg(&source_dir);
                let output = cmd.output().expect("Failed to execute git clone");
                if !output.status.success() {
                    log(LogLevel::Error, &format!("Failed to clone {} branch {} into {}", repo, branch, source_dir));
                    std::process::exit(1);
                }
            }
            #[cfg(target_os = "linux")]
            let pkg_toml = format!("{}/config_linux.toml", source_dir);
            #[cfg(target_os = "windows")]
            let pkg_toml = format!("{}/config_win32.toml", source_dir);

            let (pkg_bld_config_toml, pkg_targets_toml) = parse_config(&pkg_toml);
            log(LogLevel::Info, &format!("Parsed {}", pkg_toml));

            if pkg_bld_config_toml.packages.len() > 0 {
                for foreign_package in Package::parse_packages(&pkg_toml){
                    packages.push(foreign_package);
                }
            }

            build_config = pkg_bld_config_toml;
            build_config.compiler = build_config_toml.compiler.clone();
            build_config.build_dir = build_config_toml.build_dir.clone();
            build_config.obj_dir = build_config_toml.obj_dir.clone();
            if !Path::new(&build_config.obj_dir).exists() {
                let cmd = Command::new("mkdir")
                    .arg("-p")
                    .arg(&build_config.obj_dir)
                    .output();
                if cmd.is_err() {
                    log(LogLevel::Error, &format!("Failed to create {}", build_config.obj_dir));
                    std::process::exit(1);
                }
                log(LogLevel::Info, &format!("Created {}", build_config.obj_dir));
            }

            let tgt_configs = pkg_targets_toml;
            for mut tgt in tgt_configs {
                if tgt.typ != "dll" {
                    continue;
                }
                tgt.src = format!("{}/{}", source_dir, tgt.src).replace("\\", "/").replace("/./", "/").replace("//", "/");
                let old_inc_dir = tgt.include_dir.clone();
                tgt.include_dir = format!("./.bld_cpp/includes/{}", name).replace("\\", "/").replace("/./", "/").replace("//", "/");
                if !Path::new(&tgt.include_dir).exists() {
                    let cmd = Command::new("mkdir")
                        .arg("-p")
                        .arg(&tgt.include_dir)
                        .output();
                    if cmd.is_err() {
                        log(LogLevel::Error, &format!("Failed to create {}", tgt.include_dir));
                        std::process::exit(1);
                    }
                    log(LogLevel::Info, &format!("Created {}", tgt.include_dir));
                    let mut cm = String::new();
                    cm.push_str("cp -r ");
                    cm.push_str(&format!("{}/{}/* ", source_dir, old_inc_dir).replace("\\", "/").replace("/./", "/").replace("//", "/"));
                    cm.push_str(&tgt.include_dir);
                    cm.push_str("/ ");
                    let cmd = Command::new("sh")
                        .arg("-c")
                        .arg(&cm)
                        .output();
                    if cmd.is_err() {
                        log(LogLevel::Error, &format!("Failed to create {}", tgt.include_dir));
                        std::process::exit(1);
                    }
                }
                target_configs.push(tgt);
            }
        }

        packages.push(Package::new(name, repo, branch, build_config, target_configs));
        packages
    }
}
