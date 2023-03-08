use crate::utils::{BuildConfig, TargetConfig, log, LogLevel};
use std::path::{Path, PathBuf};
use std::io::Read;
use std::process::Command;
use std::fs;
use itertools::Itertools;
use std::collections::HashMap;

//Represents a target
pub struct Target<'a> {
    pub srcs: Vec<Src>,
    pub build_config: &'a BuildConfig,
    pub target_config: &'a TargetConfig,
    dependant_includes: HashMap<String, Vec<String>>,
}

//Represents a source file
//A single C or Cpp file
pub struct Src {
    pub path: String,
    pub name: String,
    pub obj_name: String,
    pub dependant_includes: Vec<String>,
}

impl<'a> Target<'a> {
    pub fn new(build_config: &'a BuildConfig, target_config: &'a TargetConfig) -> Self {
        let srcs = Vec::new();
        let dependant_includes: HashMap<String, Vec<String>> = HashMap::new();
        let mut target = Target {
            srcs,
            build_config,
            target_config,
            dependant_includes,
        };
        target.get_srcs(&target_config.src, target_config);
        target
    }

    pub fn build(&self) {
        for src in &self.srcs {
            if src.to_build(self.build_config) {
                src.build(self.build_config, self.target_config);
            }
        }
        self.link();
    }

    pub fn link(&self) {
        let mut objs = Vec::new();
        if !Path::new(&self.build_config.build_dir).exists() {
            fs::create_dir(&self.build_config.build_dir).unwrap();
        }
        for src in &self.srcs {
            objs.push(&src.obj_name);
        }

        let mut cmd = String::new();
        cmd.push_str(&self.build_config.compiler);
        cmd.push_str(" -o ");
        cmd.push_str(&self.build_config.build_dir);
        cmd.push_str("/");
        cmd.push_str(&self.target_config.name);

        #[cfg(target_os = "windows")]
        if self.target_config.typ == "exe" {
            cmd.push_str(".exe");
        } else if self.target_config.typ == "dll" {
            cmd.push_str(".dll");
            cmd.push_str(" -shared ");
        } else {
            log(LogLevel::Error, "Invalid target type in target config");
            log(LogLevel::Error, "  Valid types are: exe, dll");
            std::process::exit(1);
        }
        #[cfg(target_os = "linux")]
        if self.target_config.typ == "exe" {
            cmd.push_str("");
        } else if self.target_config.typ == "dll" {
            cmd.push_str(".so");
            cmd.push_str(" -shared ");
        } else {
            log(LogLevel::Error, "Invalid target type in target config");
            log(LogLevel::Error, "  Valid types are: exe, so");
            std::process::exit(1);
        }
        
        for obj in objs {
            cmd.push_str(" ");
            cmd.push_str(obj);
        }
        cmd.push_str(" ");
        cmd.push_str(&self.target_config.cflags);
        cmd.push_str(" ");
        cmd.push_str(&self.target_config.libs);


        log(LogLevel::Info, &format!("Linking target: {}", &self.target_config.name));
        log(LogLevel::Info, &format!("  Command: {}", &cmd));
        let output = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output()
            .expect("failed to execute process");
        if output.status.success() {
            log(LogLevel::Info, "  Linking successful");
        } else {
            log(LogLevel::Error, "  Linking failed");
            log(LogLevel::Error, &format!("  Error: {}", String::from_utf8_lossy(&output.stderr)));
            std::process::exit(1);
        }
    }
    //returns a vector of source files in the given root path
    fn get_srcs(&mut self, root_path: &str, target_config: &'a TargetConfig) -> Vec<Src> {
        let root_dir = PathBuf::from(root_path);
        let mut srcs : Vec<Src> = Vec::new();
        for entry in std::fs::read_dir(root_dir).unwrap() {
            let entry = entry.unwrap();
            if entry.path().is_dir() {
                let path = entry.path().to_str().unwrap().to_string();
                srcs.append(&mut self.get_srcs(&path, target_config));
            } else {
                if !entry.path().to_str().unwrap().ends_with(".cpp") && !entry.path().to_str().unwrap().ends_with(".c") {
                    continue;
                }
                let path = entry.path().to_str().unwrap().to_string().replace("\\", "/");
                self.add_src(path);
            }
        }
        srcs
    }

    //adds a source file to the target
    fn add_src(&mut self, path: String) {
        let name = Target::get_src_name(&path);
        let obj_name = self.get_src_obj_name(&name, self.build_config);
        let dependant_includes = self.get_dependant_includes(&path);
        self.srcs.push(Src::new(path, name, obj_name, dependant_includes));
    }

    //returns the file name without the extension from the path
    fn get_src_name(path: &str) -> String {
        let path_buf = PathBuf::from(path);
        let file_name = path_buf.file_name().unwrap().to_str().unwrap();
        let name = file_name.split('.').next().unwrap();
        name.to_string()
    }

    //returns the object file name for the given source file
    fn get_src_obj_name(&self, src_name: &str, build_config: &'a BuildConfig) -> String {
        let mut obj_name = String::new();
        obj_name.push_str(&build_config.obj_dir);
        obj_name.push_str("/");
        obj_name.push_str(&src_name);
        obj_name.push_str(".o");
        obj_name
    }

    //returns a vector of .h or .hpp files the given C/C++ depends on
    fn get_dependant_includes(&mut self, path: &str) -> Vec<String> {
        let mut result = Vec::new();
        let include_substrings = self.get_include_substrings(path);
        if include_substrings.len() == 0 {
            return result;
        }
        for include_substring in include_substrings {
            if self.dependant_includes.contains_key(&include_substring) {
                continue;
            }
            let mut include_path = String::new();
            include_path.push_str(&self.target_config.include_dir);
            include_path.push_str("/");
            include_path.push_str(&include_substring);
            result.append(&mut self.get_dependant_includes(&include_path));
            result.push(include_path);
            self.dependant_includes.insert(include_substring, result.clone());
        }
        let result = result.into_iter().unique().collect();
        result
    }

    //returns a vector of strings that are the include substrings
    //of the given C/C++ file as variaible path
    fn get_include_substrings(&self, path: &str) -> Vec<String> {
        let mut file = std::fs::File::open(path).unwrap();
        let mut buf = String::new();
        file.read_to_string(&mut buf).unwrap();

        let mut lines = buf.lines();
        let mut include_substrings = Vec::new();
        while let Some(line) = lines.next() {
            if line.starts_with("#include \"") {
                let include_path = line.split("\"").nth(1).unwrap().to_owned();
                include_substrings.push(include_path);
            }
        }
        include_substrings
    }
}

impl Src {
    //Creates a new source file
    fn new(path: String, name: String, obj_name: String, dependant_includes: Vec<String>) -> Self {
        Self {
            path,
            name,
            obj_name,
            dependant_includes,
        }
    }

    fn to_build(&self, build_config: &BuildConfig) -> bool {
        if !Path::new(&self.obj_name).exists() {
            log(LogLevel::Info, &format!("Building: Object file does not exist: {}", &self.obj_name));
            return true;
        }
        let obj_modified = fs::metadata(&self.obj_name).unwrap().modified().unwrap();
        let src_modified = fs::metadata(&self.path).unwrap().modified().unwrap();
        if obj_modified < src_modified {
            log(LogLevel::Info, &format!("Building: Object file is older than source file: {}", &self.obj_name));
            return true;
        }
        for dependant_include in &self.dependant_includes {
            let dependant_include_modified = fs::metadata(&dependant_include).unwrap().modified().unwrap();
            if obj_modified < dependant_include_modified {
                log(LogLevel::Info, &format!("Building: Object file is older than dependant include file: {}", &dependant_include));
                return true;
            }
        }
        log(LogLevel::Info, &format!("Building: Object file is up to date: {}", &self.obj_name));
        false
    }

    fn build(&self, build_config: &BuildConfig, target_config: &TargetConfig) {
        if !Path::new(&build_config.obj_dir).exists() {
            fs::create_dir(&build_config.obj_dir).unwrap();
        }

        let mut cmd = String::new();
        cmd.push_str(&build_config.compiler);
        cmd.push_str(" -c ");
        cmd.push_str(&self.path);
        cmd.push_str(" -o ");
        cmd.push_str(&self.obj_name);
        cmd.push_str(" -I");
        cmd.push_str(&target_config.include_dir);
        cmd.push_str(" ");
        cmd.push_str(&target_config.cflags);

        if target_config.typ == "dll" {
            cmd.push_str(" -fPIC -shared");
        }

        log(LogLevel::Info, &format!("Building: {}", &self.name));
        log(LogLevel::Info, &format!("  Command: {}", &cmd));
        let output = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output()
            .expect("failed to execute process");
        if output.status.success() {
            log(LogLevel::Info, &format!("  Success: {}", &self.name));
        } else {
            log(LogLevel::Error, &format!("  Error: {}", &self.name));
            log(LogLevel::Error, &format!("  Command: {}", &cmd));
            log(LogLevel::Error, &format!("  Stdout: {}", String::from_utf8_lossy(&output.stdout)));
            log(LogLevel::Error, &format!("  Stderr: {}", String::from_utf8_lossy(&output.stderr)));
        }
    }
}
