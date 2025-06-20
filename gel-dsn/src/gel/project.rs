use sha1::{Digest, Sha1};
use std::{
    ffi::{OsStr, OsString},
    io,
    path::{Path, PathBuf},
};

use crate::{
    gel::{context_trace, DatabaseBranch},
    FileAccess,
};

use super::{BuildContext, BuildContextImpl, InstanceName};

/// The ordered list of project filenames supported.
pub const PROJECT_FILES: &[&str] = &["gel.toml", "edgedb.toml"];

#[derive(Debug)]
#[allow(unused)]
pub struct ProjectSearchResult {
    /// The path to the project file (gel.toml or edgedb.toml)
    pub project_path: PathBuf,
    /// The path to the project's stash file (~/.config/{edgedb,gel}/projects/{name}-{hash})
    pub stash_path: PathBuf,
    /// The project metadata
    pub project: Option<Project>,
}

impl ProjectSearchResult {
    /// Find a project in the given directory.
    pub fn find(dir: ProjectDir) -> std::io::Result<Option<Self>> {
        let context = BuildContextImpl::new();
        let project = find_project_file(&context, dir)?;
        Ok(project)
    }
}

pub enum ProjectDir {
    /// Search the current directory (include parents).
    SearchCwd,
    /// Search the given path (include paths).
    Search(PathBuf),
    /// Check the given path without searching parents.
    NoSearch(PathBuf),
    /// Assume the given path is a valid project file.
    Exact(PathBuf),
}

impl ProjectDir {
    pub fn search_parents(&self) -> bool {
        match self {
            ProjectDir::Search(_) => true,
            ProjectDir::NoSearch(_) => false,
            ProjectDir::Exact(_) => false,
            ProjectDir::SearchCwd => true,
        }
    }
}

/// Searches for a project file either from the current directory or exact path.
pub fn find_project_file(
    context: &impl BuildContext,
    start_path: ProjectDir,
) -> io::Result<Option<ProjectSearchResult>> {
    let project_path = if let ProjectDir::Exact(path) = start_path {
        path
    } else {
        let search_parents = start_path.search_parents();
        let dir = match start_path {
            ProjectDir::SearchCwd => {
                let Some(cwd) = context.cwd() else {
                    context_trace!(context, "No current directory, skipping project search");
                    return Ok(None);
                };
                cwd.to_path_buf()
            }
            ProjectDir::Search(path) => path,
            ProjectDir::NoSearch(path) => path,
            ProjectDir::Exact(..) => unreachable!(),
        };
        let Some(project_path) = search_directory(context, &dir, search_parents)? else {
            context_trace!(context, "No project file found");
            return Ok(None);
        };
        context_trace!(context, "Project file path: {:?}", project_path);
        project_path
    };
    context_trace!(context, "Project path: {:?}", project_path);
    let stash_path = match get_stash_path(context, project_path.parent().unwrap_or(&project_path)) {
        Ok(stash_path) => stash_path,
        Err(e) => {
            // Special handling -- NotFound is mapped to Ok(None)
            if e.kind() == io::ErrorKind::NotFound {
                return Ok(None);
            }
            context_trace!(
                context,
                "Error getting the stash path: {e:?} for project path: {project_path:?}"
            );
            return Err(e);
        }
    };
    context_trace!(context, "Stash path: {:?}", stash_path);
    let project = Project::load(&stash_path, context);
    context_trace!(context, "Project: {:?}", project);
    Ok(Some(ProjectSearchResult {
        project_path,
        stash_path,
        project,
    }))
}

/// Computes the SHA-1 hash of a path's canonical representation.
fn hash_path(path: &Path) -> String {
    let mut hasher = Sha1::new();
    hasher.update(path.as_os_str().as_encoded_bytes());
    format!("{:x}", hasher.finalize())
}

/// Generates a stash name for a project directory.
fn stash_name(path: &Path) -> OsString {
    let hash = hash_path(path);
    let base = path.file_name().unwrap_or(OsStr::new(""));
    let mut name = base.to_os_string();
    name.push("-");
    name.push(hash);
    name
}

/// Searches for project files in the given directory and optionally its parents.
fn search_directory(
    context: &impl BuildContext,
    base: &Path,
    search_parents: bool,
) -> io::Result<Option<PathBuf>> {
    let mut path = base.to_path_buf();
    loop {
        let mut found = Vec::new();
        for name in PROJECT_FILES {
            let file = path.join(name);
            if context.files().exists(&file)? {
                context_trace!(context, "Found project file: {:?}", file);
                found.push(file);
            }
        }

        if found.len() > 1 {
            let (first, rest) = found.split_at(1);
            let first_content = context.files().read(&first[0])?;
            for file in rest {
                if context.files().read(file)? != first_content {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "{:?} and {:?} found in {:?} but the contents are different",
                            first[0].file_name(),
                            file.file_name(),
                            path
                        ),
                    ));
                }
            }
            return Ok(Some(first[0].clone()));
        } else if let Some(file) = found.pop() {
            return Ok(Some(file));
        }

        if !search_parents {
            break;
        }
        if let Some(parent) = path.parent() {
            path = parent.to_path_buf();
        } else {
            break;
        }
    }
    Ok(None)
}

/// Computes the path to the project's stash file based on the canonical path.
fn get_stash_path(context: &impl BuildContext, project_dir: &Path) -> io::Result<PathBuf> {
    let canonical = context
        .files()
        .canonicalize(project_dir)
        .unwrap_or(project_dir.to_path_buf());
    let stash_name = stash_name(&canonical);
    context_trace!(context, "Stash name: {:?}", stash_name);
    let path = Path::new("projects").join(stash_name);
    context.find_config_path(path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub cloud_profile: Option<String>,
    pub instance_name: InstanceName,
    pub project_path: Option<PathBuf>,
    pub branch: Option<String>,
    pub database: Option<String>,
}

impl Project {
    /// Returns the database or branch for the project.
    pub fn db(&self) -> DatabaseBranch {
        match (self.branch.clone(), self.database.clone()) {
            (Some(branch), Some(_database)) => DatabaseBranch::Ambiguous(branch),
            (Some(branch), None) => DatabaseBranch::Branch(branch),
            (None, Some(database)) => DatabaseBranch::Database(database),
            (None, None) => DatabaseBranch::Default,
        }
    }
}

impl Project {
    #[cfg(test)]
    pub fn new(instance_name: InstanceName) -> Self {
        Self {
            cloud_profile: None,
            instance_name,
            project_path: None,
            branch: None,
            database: None,
        }
    }

    pub(crate) fn load(path: &Path, context: &impl BuildContext) -> Option<Self> {
        let cloud_profile = context
            .read_config_file::<String>(&path.join("cloud-profile"))
            .unwrap_or_default();
        let instance_name = context
            .read_config_file::<InstanceName>(&path.join("instance-name"))
            .unwrap_or_default();
        let project_path = context
            .read_config_file::<PathBuf>(&path.join("project-path"))
            .unwrap_or_default();
        let branch = context
            .read_config_file::<String>(&path.join("branch"))
            .unwrap_or_default();
        let database = context
            .read_config_file::<String>(&path.join("database"))
            .unwrap_or_default();
        let instance_name = instance_name?;
        Some(Self {
            cloud_profile,
            instance_name,
            project_path,
            branch,
            database,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        file::SystemFileAccess,
        gel::{BuildContextImpl, Traces},
    };
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_stash_examples() {
        let files = HashMap::from_iter([
            (Path::new("/home/edgedb/test/gel.toml"),
            ""),
            (Path::new("/home/edgedb/.config/edgedb/projects/test-cf3c86df8fc33fbb73a47671ac5762eda8219158/instance-name"),
            "instance-name"),
        ]);

        let traces = Traces::default();

        let mut context = BuildContextImpl::new_with((), files);
        context.logging.tracing = Some(traces.clone().trace_fn());
        context.paths.config_dirs = vec![PathBuf::from("/home/edgedb/.config/edgedb")];
        let res = find_project_file(
            &context,
            ProjectDir::Search(PathBuf::from("/home/edgedb/test")),
        );

        for trace in traces.into_vec() {
            eprintln!("{trace}");
        }
        let res = res.unwrap().unwrap();
        assert_eq!(
            res.project_path,
            PathBuf::from("/home/edgedb/test/gel.toml")
        );
        assert_eq!(
            res.project,
            Some(Project::new(InstanceName::Local(
                "instance-name".to_string()
            )))
        );
    }

    #[test]
    fn test_project_file_priority() {
        use std::fs;

        let temp = tempfile::tempdir().unwrap();
        let base = temp.path();

        let gel_path = base.join("gel.toml");
        let edgedb_path = base.join("edgedb.toml");
        let config_dir = base.join("config");
        std::fs::create_dir_all(config_dir.join("projects")).unwrap();

        let mut context = BuildContextImpl::new_with((), SystemFileAccess);
        context.paths.config_dirs = vec![config_dir];

        // Test gel.toml only
        fs::write(&gel_path, "test1").unwrap();
        let found = find_project_file(&context, ProjectDir::Search(base.to_path_buf()))
            .unwrap()
            .unwrap();
        assert_eq!(found.project_path, gel_path);

        // Test edgedb.toml only
        fs::remove_file(&gel_path).unwrap();
        fs::write(&edgedb_path, "test2").unwrap();
        let found = find_project_file(&context, ProjectDir::Search(base.to_path_buf()))
            .unwrap()
            .unwrap();
        assert_eq!(found.project_path, edgedb_path);

        // Test both files with same content
        fs::write(&gel_path, "test3").unwrap();
        fs::write(&edgedb_path, "test3").unwrap();
        let found = find_project_file(&context, ProjectDir::Search(base.to_path_buf()))
            .unwrap()
            .unwrap();
        assert_eq!(found.project_path, gel_path);

        // Test both files with different content
        fs::write(&gel_path, "test4").unwrap();
        fs::write(&edgedb_path, "test5").unwrap();
        let err = find_project_file(&context, ProjectDir::Search(base.to_path_buf())).unwrap_err();
        assert!(err.to_string().contains("but the contents are different"));
    }
}
