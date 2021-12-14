use std::path::Path;

use smash_arc::*;

use crate::{resource::LoadedFilepath, replacement::FileInfoFlagsExt, PathExtension};

use super::{AdditionContext, FromPathExt, SearchContext, SearchEx};

pub fn add_file(ctx: &mut AdditionContext, path: &Path) {
    let mut file_path = if let Some(file_path) = FilePath::from_path(path) {
        file_path
    } else {
        error!("Failed to generate a FilePath from {}!", path.display());
        return;
    };

    let filepath_idx = FilePathIdx(ctx.filepaths.len() as u32);
    let file_info_indice_idx = FileInfoIndiceIdx(ctx.file_info_indices.len() as u32);
    let file_info_idx = FileInfoIdx(ctx.file_infos.len() as u32);
    let info_to_data_idx = InfoToDataIdx(ctx.info_to_datas.len() as u32);
    let file_data_idx = FileDataIdx(ctx.file_datas.len() as u32);

    let base_file = ctx.get_file_in_folder(ctx.get_file_info_from_hash(Hash40::from("fighter/mario/model/body/c00/model.numdlb")).unwrap(), Region::None);

    let new_info_indice_idx = FileInfoIndex {
        dir_offset_index: 0xFF_FFFF,
        file_info_index: file_info_idx
    };

    let mut new_file_info = FileInfo {
        file_path_index: filepath_idx,
        file_info_indice_index: file_info_indice_idx,
        info_to_data_index: info_to_data_idx,
        flags: FileInfoFlags::new().with_unknown1(true)
    };
    new_file_info.flags.set_standalone_file(true);

    let new_info_to_data = FileInfoToFileData {
        folder_offset_index: base_file.folder_offset_index,
        file_data_index: file_data_idx,
        file_info_index_and_load_type: FileInfoToFileDataBitfield::new().with_load_type(1)
    };

    let base_file = ctx.get_file_datas()[usize::from(base_file.file_data_index)];

    let new_file_data = FileData {
        offset_in_folder: base_file.offset_in_folder,
        comp_size: base_file.comp_size,
        decomp_size: base_file.decomp_size,
        flags: FileDataFlags::new().with_compressed(false).with_use_zstd(false)
    };

    file_path.path.set_index(file_info_indice_idx.0);

    ctx.filepaths.push(file_path);
    ctx.file_info_indices.push(new_info_indice_idx);
    ctx.file_infos.push(new_file_info);
    ctx.info_to_datas.push(new_info_to_data);
    ctx.file_datas.push(new_file_data);

    ctx.loaded_filepaths.push(LoadedFilepath::default());
    ctx.loaded_datas.reserve(1);

    info!("Added fle '{}' ({:#x})", path.display(), file_path.path.hash40().0);
}

pub fn add_searchable_folder_recursive(ctx: &mut SearchContext, path: &Path) {
    let (parent, current_path_list_indices_len) = match path.parent() {
        Some(parent) if parent == Path::new("") => {
            if let Some(mut new_folder_path) = FolderPathListEntry::from_path(path) {
                let new_path = new_folder_path.as_path_entry();
                new_folder_path.set_first_child_index(0xFF_FFFF);
                ctx.new_folder_paths.insert(new_folder_path.path.hash40(), ctx.folder_paths.len());
                ctx.new_paths.insert(new_path.path.hash40(), ctx.path_list_indices.len());
                ctx.path_list_indices.push(ctx.paths.len() as u32);
                ctx.paths.push(new_path);
                ctx.folder_paths.push(new_folder_path);
                return;
            } else {
                error!("Unable to generate new folder path list entry for {}", path.display());
                return;
            }
        }
        Some(parent) => match parent.smash_hash() {
            Ok(hash) => {
                let len = ctx.path_list_indices.len();
                match ctx.get_folder_path_mut(hash) {
                    Some(parent) => {
                        (parent, len)
                    },
                    None => {
                        add_searchable_folder_recursive(ctx, parent);
                        let len = ctx.path_list_indices.len();
                        match ctx.get_folder_path_mut(hash) {
                            Some(parent) => (parent, len),
                            None => {
                                error!("Unable to add folder '{}'", parent.display());
                                return;
                            }
                        }
                    }
                }
            },
            Err(e) => {
                error!("Unable to get the smash hash for '{}'. {:?}", parent.display(), e);
                return;
            }
        },
        None => {
            error!("Failed to get the parent for path '{}'", path.display());
            return;
        }
    };

    if let Some(mut new_folder) = FolderPathListEntry::from_path(path) {
        new_folder.set_first_child_index(0xFF_FFFF);
        let mut new_path = new_folder.as_path_entry();
        parent.set_first_child_index(current_path_list_indices_len as u32);
        new_path.path.set_index(parent.get_first_child_index() as u32);
        drop(parent);
        ctx.new_folder_paths.insert(new_folder.path.hash40(), ctx.folder_paths.len());
        ctx.new_paths.insert(new_path.path.hash40(), ctx.path_list_indices.len());
        ctx.path_list_indices.push(ctx.paths.len() as u32);
        ctx.folder_paths.push(new_folder);
        ctx.paths.push(new_path);
    } else {
        error!("Failed to add folder {}!", path.display());
    }
}

pub fn add_searchable_file_recursive(ctx: &mut SearchContext, path: &Path) {
    let (parent, current_path_list_indices_len) = match path.parent() {
        Some(parent) if parent == Path::new("") => {
            error!("Cannot add file {} as root file!", path.display());
            return;
        }
        Some(parent) => match parent.smash_hash() {
            Ok(hash) => {
                let len = ctx.path_list_indices.len();
                match ctx.get_folder_path_mut(hash) {
                    Some(parent) => {
                        (parent, len)
                    },
                    None => {
                        add_searchable_folder_recursive(ctx, parent);
                        let len = ctx.path_list_indices.len();
                        match ctx.get_folder_path_mut(hash) {
                            Some(parent) => (parent, len),
                            None => {
                                error!("Unable to add folder '{}'", parent.display());
                                return;
                            }
                        }
                    }
                }
            },
            Err(e) => {
                error!("Unable to get the smash hash for '{}'. {:?}", parent.display(), e);
                return;
            }
        },
        None => {
            error!("Failed to get the parent for path '{}'", path.display());
            return;
        }
    };

    if let Some(mut new_file) = PathListEntry::from_path(path) {
        parent.set_first_child_index(current_path_list_indices_len as u32);
        new_file.path.set_index(parent.get_first_child_index() as u32);
        drop(parent);
        ctx.new_paths.insert(new_file.path.hash40(), ctx.path_list_indices.len());
        ctx.path_list_indices.push(ctx.paths.len() as u32);
        ctx.paths.push(new_file);
    } else {
        error!("Failed to add folder {}!", path.display());
    }
}