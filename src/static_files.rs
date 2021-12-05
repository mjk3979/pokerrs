use std::collections::HashMap;
use std::fs::File;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

#[derive(Debug)]
pub struct StaticFiles {
    paths: HashMap<String, PathBuf>
}

fn remove_root(root: &str, mut path: String) -> String {
    let mut retval = path.split_off(root.len());
    while retval.starts_with("/") || retval.starts_with("\\") {
        retval.remove(0);
    }
    str::replace(&retval, "\\", "/")
}

pub fn content_type(path: &str) -> &'static str {
    if path.ends_with(".js") {
        "text/javascript"
    } else {
        ""
    }
}

impl StaticFiles {
    pub fn from_dir_path(root: &str) -> StaticFiles {
        let mut paths = HashMap::new();
        let mut directories: Vec<PathBuf> = vec![root.clone().into()];
        while let Some(dirpath) = directories.pop() {
            for entry in fs::read_dir(dirpath).unwrap().filter_map(|e| e.ok()) {
                if let Ok(ftype) = entry.file_type() {
                    if ftype.is_symlink() || ftype.is_dir() {
                        directories.push(entry.path());
                    } else if ftype.is_file() {
                        let path = entry.path();
                        if let Ok(s) = path.clone().into_os_string().into_string() {
                            paths.insert(remove_root(root, s), path);
                        }
                    }
                }
            }
        }
        StaticFiles { paths }
    }

    pub fn load_file(&self, path: &str) -> Option<Vec<u8>> {
        if let Some(filepath) = self.paths.get(path) {
            let mut buffer = Vec::new();
            File::open(filepath).ok().map(|mut f| f.read_to_end(&mut buffer).ok().map(|_| buffer)).flatten()
        } else {
            None
        }
    }
}
