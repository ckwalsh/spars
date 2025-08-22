// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use compact_str::format_compact;
use compact_str::CompactString;
use thiserror::Error;
use zerotrie::ZeroTrieBuildError;
use zerotrie::ZeroTrieSimpleAscii;

use crate::response::Response;
use crate::response::StatusCode;
use crate::settings::ExposeHiddenFiles;
use crate::settings::HandlerSettings;

mod mime;
pub struct Handler {
    file_data: Vec<HandlerFileData>,
    mime_types: Vec<&'static str>,
    path_data: Vec<HandlerPathData>,
    fallback_idx: Option<usize>,
    path_trie: ZeroTrieSimpleAscii<Vec<u8>>,
}

struct HandlerFileData {
    resolved_path: Arc<Path>,
    len: u64,
}

enum HandlerPathData {
    Found {
        file_idx: usize,
        mime_idx: Option<usize>,
    },
    DirRedirect(Arc<str>),
}

#[derive(Debug, Error)]
pub enum HandlerBuildError {
    #[error("Loop while walking paths")]
    PathLoop,

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error("Non-Utf8 path component")]
    NonUtf8Component(OsString),

    #[error("Non-ascii path component: {0}")]
    NonAsciiComponent(CompactString),

    #[error(transparent)]
    ZeroTrieBuildError(#[from] ZeroTrieBuildError),
}

impl Handler {
    pub fn build_from_root(
        mut root: PathBuf,
        index_file: &str,
        fallback_path: Option<&str>,
        expose_hidden: ExposeHiddenFiles,
    ) -> Result<Self, HandlerBuildError> {
        let mut file_data: Vec<HandlerFileData> = Vec::new();
        let mut mime_types: Vec<&'static str> = Vec::new();
        let mut path_data: Vec<HandlerPathData> = Vec::new();

        let mut file_index: BTreeMap<PathBuf, usize> = BTreeMap::new();
        let mut mime_index: BTreeMap<&'static str, usize> = BTreeMap::new();

        let mut entries: Vec<(CompactString, usize)> = Vec::new();

        let mut dirs_searched: BTreeSet<PathBuf> = BTreeSet::new();
        let mut search: Vec<(CompactString, PathBuf)> = Vec::new();

        {
            root = root.canonicalize()?;
            let metadata = root.metadata()?;

            if !metadata.is_dir() {
                return Err(HandlerBuildError::IoError(
                    std::io::ErrorKind::NotADirectory.into(),
                ));
            }

            dirs_searched.insert(root.clone());
            search.push(("/".into(), root));
        }

        while let Some((http_prefix, dir_path)) = search.pop() {
            for entry in std::fs::read_dir(dir_path)? {
                let entry = entry?;
                let original_path = entry.path();
                let resolved_path = original_path.canonicalize()?;

                if dirs_searched.contains(resolved_path.as_path()) {
                    return Err(HandlerBuildError::PathLoop);
                }

                let file_name = entry.file_name();

                let http_name = if let Some(s) = file_name.to_str() {
                    if s.is_ascii() {
                        s
                    } else {
                        return Err(HandlerBuildError::NonAsciiComponent(s.into()));
                    }
                } else {
                    return Err(HandlerBuildError::NonUtf8Component(file_name));
                };

                if http_name.starts_with('.') {
                    match expose_hidden {
                        ExposeHiddenFiles::OnlyWellKnown => {
                            if (http_prefix.is_empty() && http_name == ".well-known")
                                || http_prefix == "/.well-known/"
                            {
                                // All good
                            } else {
                                continue;
                            }
                        }
                        ExposeHiddenFiles::Hide => continue,
                        ExposeHiddenFiles::Expose => (),
                    }
                }

                let http_path = format_compact!("{http_prefix}{http_name}");

                let file_idx = match file_index.get(&resolved_path) {
                    Some(&file_idx) => file_idx,
                    None => {
                        let metadata = entry.metadata()?;

                        if metadata.is_dir() {
                            dirs_searched.insert(resolved_path.clone());
                            search.push((
                                format_compact!("{http_prefix}{http_name}/"),
                                resolved_path,
                            ));
                            continue;
                        }

                        let file_idx = file_data.len();

                        file_data.push(HandlerFileData {
                            resolved_path: Arc::from(resolved_path.as_path()),
                            len: metadata.len(),
                        });

                        file_index.insert(resolved_path, file_idx);

                        file_idx
                    }
                };

                let mime_idx = mime::mime_from_path(&original_path).map(|mime_type| {
                    *mime_index.entry(mime_type).or_insert_with(|| {
                        let idx = mime_types.len();
                        mime_types.push(mime_type);
                        idx
                    })
                });

                let path_idx = path_data.len();

                path_data.push(HandlerPathData::Found { file_idx, mime_idx });

                entries.push((http_path, path_idx));

                if http_name == index_file {
                    entries.push((http_prefix.clone(), path_idx));

                    path_data.push(HandlerPathData::DirRedirect(Arc::from(
                        http_prefix.as_ref(),
                    )));
                    entries.push((http_prefix.trim_end_matches('/').into(), path_idx + 1));
                }
            }
        }

        let path_trie: ZeroTrieSimpleAscii<Vec<u8>> = entries.into_iter().collect();

        let fallback_idx = fallback_path.and_then(|path| path_trie.get(path));

        Ok(Self {
            path_trie,
            path_data,
            file_data,
            mime_types,
            fallback_idx,
        })
    }

    pub fn handle(&self, method: Option<&str>, path: Option<&str>) -> Result<Response, Response> {
        let send_body = match method {
            Some("HEAD") => false,
            Some("GET") => true,
            Some(_) | None => {
                return Ok(Response::StatusStr(StatusCode::METHOD_NOT_ALLOWED));
            }
        };

        let (path, query_sep, query) = match path {
            Some(path) => {
                if let Some((path, query)) = path.split_once('?') {
                    (path, "?", query)
                } else {
                    (path, "", "")
                }
            }
            None => {
                return Ok(Response::StatusStr(StatusCode::BAD_REQUEST));
            }
        };

        let path_idx = match self.path_trie.get(path).or(self.fallback_idx) {
            Some(idx) => idx,
            None => return Ok(Response::NotFound),
        };

        let path_data = match self.path_data.get(path_idx) {
            Some(data) => data,
            None => {
                return Ok(Response::StatusStr(StatusCode::INTERNAL_SERVER_ERROR));
            }
        };

        let (file_data, mime_idx) = match path_data {
            HandlerPathData::Found { file_idx, mime_idx } => match self.file_data.get(*file_idx) {
                Some(data) => (data, mime_idx),
                None => return Ok(Response::NotFound),
            },
            HandlerPathData::DirRedirect(path) => {
                let len = path.len() + query_sep.len() + query.len();

                if len > 256 {
                    return Ok(Response::StatusStr(StatusCode::URI_TOO_LONG));
                } else {
                    return Ok(Response::Redirect {
                        path: Arc::clone(path),
                        query: format_compact!("{query_sep}{query}"),
                    });
                }
            }
        };

        let mime_type = mime_idx.and_then(|mime_idx| self.mime_types.get(mime_idx).cloned());

        let resolved_path = if send_body {
            Some(Arc::clone(&file_data.resolved_path))
        } else {
            None
        };

        Ok(Response::Found {
            resolved_path,
            len: file_data.len,
            mime_type,
        })
    }
}

impl TryFrom<HandlerSettings> for Handler {
    type Error = HandlerBuildError;

    fn try_from(settings: HandlerSettings) -> Result<Self, Self::Error> {
        Self::build_from_root(
            settings.root,
            settings.index_file.as_str(),
            settings.fallback_path.as_deref(),
            settings.expose_hidden,
        )
    }
}
