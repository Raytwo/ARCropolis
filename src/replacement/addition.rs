use std::{collections::HashSet, path::Path};

use arc_config::{
    search::{File, Folder},
    ToSmashArc,
};
use smash_arc::*;
use thiserror::Error;

use super::{lookup, AdditionContext, FromPathExt, FromSearchableFile, FromSearchableFolder, SearchContext, DirInfoExt};
use crate::{
    hashes,
    replacement::FileInfoFlagsExt,
    resource::LoadedFilepath,
    PathExtension, HashingError,
};

#[derive(Debug, Error)]
pub enum FilePathError {
    #[error("failed to generate a FilePath")]
    Hash(#[from] HashingError),
}

#[derive(Debug, Error)]
pub enum FolderPathListEntryError {
    #[error("failed to generate a FolderPathListEntry")]
    Hash(#[from] HashingError),
}

#[derive(Debug, Error)]
pub enum DirectoryAdditionError {
    #[error("couldn't add the directory for reasons but hopefully it'll be clearer")]
    AdditionFailed,
    #[error("hashing error")]
    Hash(#[from] HashingError),
    #[error("lookup error")]
    Lookup(#[from] LookupError),
    #[error("directory already exists")]
    AlreadyExists(DirInfo),
}

#[derive(Debug, Error)]
pub enum OutOfBounds {
    #[error("File Paths index out of bounds - Index: {0}")]
    FilePath(usize),
    #[error("File Info Indices index out of bounds - Index: {0}")]
    FileInfoIndex(usize),
    #[error("File Infos index out of bounds - Index: {0}")]
    FileInfo(usize),
    #[error("Info to Data index out of bounds - Index: {0}")]
    InfoToData(usize),
    #[error("File Data index out of bounds - Index: {0}")]
    FileData(usize),
    #[error("File Infos range out of bounds - min: {0}, max: {1}")]
    FileInfos(usize, usize),
}

#[derive(Debug, Error)]
pub enum FolderAddition {
    #[error("folder has no parent")]
    NoParent,
    #[error("folder has no name")]
    NoName,
    #[error("hashing error - {0}")]
    Hash(#[from] HashingError),
}

#[derive(Debug, Error)]
pub enum FileAdditionError {
    #[error("path has no parent")]
    MissingParent,
    #[error("hashing error - {0}")]
    Hash(#[from] HashingError),
    #[error("lookup error - {0}")]
    Lookup(#[from] LookupError),
    #[error("directory addition error - {0}")]
    Directory(#[from] DirectoryAdditionError), // lol, lmao
    #[error("folder addition error - {0}")]
    Folder(#[from] FolderAddition), // lol, lmao
    #[error("out of bounds error - {0}")]
    OutOfBounds(#[from] OutOfBounds),
}

pub fn add_file(ctx: &mut AdditionContext, path: &Path) -> Result<(), FilePathError> {
    // Create a base file for the new file from mario's numdlb and set the region to none
    let base_file = ctx.get_file_in_folder(
        ctx.get_file_info_from_hash(Hash40::from("fighter/mario/model/body/c00/model.numdlb"))
            .unwrap(),
        Region::None,
    );

    // Redeclare base_file with the file data from fhe file data index of the base file
    let base_filedata = ctx.get_file_datas()[usize::from(base_file.file_data_index)];

    // Create a new FileData with the base_file's FileData information
    let new_file_dataidx = ctx.new_file_data(base_filedata.offset_in_folder, base_filedata.comp_size, base_filedata.decomp_size);

    // Create new FileInfoToData with the folder_offset_index of the base file from earlier, file_data_index from the newly generated file_data_idx above, and a file info index and load type of 1
    let new_info_to_data_idx = ctx.new_fileinfo_to_filedata(base_file.folder_offset_index, new_file_dataidx);

    // Create a new FileInfoIndex with the created file_info_idx above and a dir offset index of
    ctx.new_fileinfo_from_path(path, new_info_to_data_idx)?;

    // Push default values to the loaded_(filepaths/datas) to make it match up with the other vectors length
    ctx.expand_file_resource_tables();

    // info!("Added file '{}'", path.display());
    Ok(())
}

pub fn add_shared_file(ctx: &mut AdditionContext, new_file: &File, shared_to: Hash40) -> Result<(), LookupError> {
    // Get the target shared FileInfoIndice index
    let info_indice_idx = ctx.get_file_info_indice_idx(shared_to)?;

    // Make FilePath from path passed in
    let mut filepath = FilePath::from_file(new_file);    

    let shared_to = ctx.add_shared_filepath(filepath, info_indice_idx);

    // Add the shared file to the lookup
    lookup::add_shared_file(
        new_file.full_path.to_smash_arc(), // we can unwrap because of FilePath::from_path being successful
        shared_to.to_smash_arc(),
    );

    Ok(())
}

pub fn add_shared_searchable_file(ctx: &mut SearchContext, new_file: &File) -> Result<(), FileAdditionError> {
    ctx.add_shared_searchable_file(new_file)
    // match ctx.get_folder_path(new_file.parent.full_path.to_smash_arc()) {
    //     Some(mut parent) => {
    //         let mut new_file = PathListEntry::from_file(new_file);
    //         ctx.add_new_search_path_with_parent(new_file, &mut parent);
    //         Ok(())
    //     },
    //     None => {
    //         add_searchable_folder_by_folder(ctx, &new_file.parent)?;
    //         add_shared_searchable_file(ctx, new_file)
    //     },
    // }

        // new_file.path.set_index(parent.get_first_child_index() as u32);
        // // Set the file as the head of the linked list
        // parent.set_first_child_index(path_list_indices_len as u32);
        // ctx.new_paths.insert(new_file.path.hash40(), ctx.path_list_indices.len());
        // ctx.path_list_indices.push(ctx.paths.len() as u32);
        // ctx.paths.push(new_file);

    // // if it isn't there, then we are going to recursively add it's parent, returning out if it's not possible
    // if !has_parent && !add_searchable_folder_by_folder(ctx, &new_file.parent).is_ok() {
    //     // error!("Cannot add shared file to search section because we could not add it's parents");
    //     return Err(FileAdditionError::Folder(FolderAddition::NoParent));
    // }

    // // we get the length here because we are about to acquire a mutable reference
    // // to the parent folder, and we cannot get the length via immutable reference
    // // if that one is active
    // let path_list_indices_len = ctx.path_list_indices.len();

    // let Some(parent) = ctx.get_folder_path_mut(new_file.parent.full_path.to_smash_arc()) else {
    //     error!("Cannot add shared file to search section because its parent does not exist");
    //     return Err(FileAdditionError::Folder(FolderAddition::NoParent));
    // };
    
    // Ok(())
}

pub fn add_searchable_file(ctx: &mut SearchContext, path: &Path) -> Result<(), FileAdditionError> {
    // let parent: &Path = path.parent().unwrap_or_else(|| panic!("Failed to get the parent for path '{}'", path.display()));

    let Some(parent) = path.parent() else {
        return Err(FileAdditionError::MissingParent);
    };

    // If the parent is empty, then just return
    if parent == Path::new("") {
        // error!("Cannot add file {} as root file!", path.display());
        return Err(FileAdditionError::MissingParent);
    }
    
    // If the parent is alright (actually something), keep going

    // Convert the parent to a smash_hash
    let hash = parent.smash_hash()?;
    
    // Get the length of the current path list indices
    let len = ctx.path_list_indices.len();

    // Create a parent and current_path_list_indices_len variable tuple
    let (parent, current_path_list_indices_len) = match ctx.get_folder_path_mut(hash) {
        // If the folder path exists, then just return the parent and the length
        Some(parent) => (parent, len),
        None => {
            // If it doesn't exist, then call the add_searchable_folder_recursive with the parent path
            ctx.add_folder_recursive(parent).unwrap();
            
            // Get the new length since it changed thanks to the previous function call
            let len = ctx.path_list_indices.len();

            // Try getting the folder path again
            ctx.get_folder_path_mut(hash)
                .map(|parent| (parent, len))
                .ok_or(FileAdditionError::Lookup(LookupError::Missing))?
        },
    };

    // Try getting the file from the path after adding the folders
    let mut new_file = PathListEntry::from_path(path)?; // error!("Failed to add folder {}!", path.display());
    
    // Set the previous head of the linked list as the child of the new file
    new_file.path.set_index(parent.get_first_child_index() as u32);
    
    // Set the file as the head of the linked list
    parent.set_first_child_index(current_path_list_indices_len as u32);
    ctx.new_paths.insert(new_file.path.hash40(), ctx.path_list_indices.len());
    ctx.path_list_indices.push(ctx.paths.len() as u32);
    ctx.paths.push(new_file);

    Ok(())
}

pub fn set_file_info_for_dir_insertion(ctx: &mut AdditionContext, file_info: &mut FileInfo, file_path_idx: &FilePathIdx) -> Result<(), FileAdditionError> {
    if !file_info.flags.new_shared_file() {
        let info_to_data = &mut ctx.get_file_info_to_file_data_from_fileinfo(&file_info)?;
        info_to_data.folder_offset_index = 0x0;

        let file_data = &mut ctx.get_file_data_from_file_data_index(&info_to_data.file_data_index)?;
        ctx.reset_file_data(file_data);
    }

    file_info.file_path_index = *file_path_idx;
    file_info.flags.set_standalone_file(true);

    Ok(())
}

pub fn add_files_to_directory(ctx: &mut AdditionContext, directory: Hash40, files: HashSet<Hash40>) -> Result<(), FileAdditionError> {
    // Get the file info range for the dir info that was passed in
    let file_info_range = ctx.get_dir_info_from_hash_ctx(directory)?.file_info_range();

    // Create new HashSet for the current files in the specified dir
    let mut contained_files = HashSet::new();

    // Create new Vec with the size of the file count of the specified dir + the count of the files passed in
    let mut new_file_infos = Vec::with_capacity(file_info_range.len() + files.len());

    // Get all the previous file_infos associated with the dir info

    match ctx.get_file_infos_from_range(&file_info_range) {
        // If the directory already has a file info range, then we get the previous ones and add them to the vec
        Ok(file_infos) => {
            // Go through all the previous file infos and add them to the new file infos
            file_infos.iter().try_for_each(|file_info| -> Result<(), FileAdditionError> {
                let filepath = ctx.get_filepath_from_file_path_idx(&file_info.file_path_index)?;
                let file_info_index = &mut ctx.get_file_info_index_from_filepath(&filepath)?;

                contained_files.insert(filepath.path.hash40());

                if file_info_range.contains(&usize::from(file_info_index.file_info_index)) {
                    file_info_index.file_info_index = FileInfoIdx((ctx.file_infos.len() + new_file_infos.len()) as u32);
                }

                new_file_infos.push(*file_info);

                Ok(())
            })?;
        },
        Err(err) => {}
    }

    // Filter for new files that have an index
    let new_file_path_idxs = files.iter()
        .filter(|file| !contained_files.contains(&file))
        .filter_map(|file| ctx.get_path_idx(file))
        .collect::<Vec<_>>();

    // Add new files indexes to the new_files_info vec    
    new_file_path_idxs.iter().try_for_each(|file_idx| -> Result<(), FileAdditionError> {
        let filepath = ctx.get_filepath_from_file_path_idx(&file_idx)?;
        
        let file_info_index = &mut ctx.get_file_info_index_from_filepath(&filepath)?;
        file_info_index.file_info_index = FileInfoIdx((ctx.file_infos.len() + new_file_infos.len()) as u32);

        let file_info = &mut ctx.get_file_info_from_file_info_index(&file_info_index)?;
        set_file_info_for_dir_insertion(ctx, file_info, &file_idx)?;
        
        new_file_infos.push(*file_info);
        Ok(())
    })?;
    
    let file_start_index = ctx.file_infos.len() as u32;
    let file_count = new_file_infos.len() as u32;
    ctx.update_directory_info_files(directory, file_start_index, file_count)?;

    Ok(())
}

pub fn add_dir_info(ctx: &mut AdditionContext, path: &Path) -> Result<DirInfo, DirectoryAdditionError> {
    // Create a FolderPathListEntry from the path that's passed in
    let dir_info_path = FolderPathListEntry::from_path(path)?;

    // If the dir info already exists, then just go back and give it the found dir info
    if let Ok(res) = ctx.get_dir_info_from_hash_ctx(dir_info_path.path.hash40()) {
        return Err(DirectoryAdditionError::AlreadyExists(*res))
    }

    // Make a new dir info based on the path passed in
    let mut dir_info = ctx.new_dir_info(dir_info_path)?;

    // Make a new hash to info idx using the dir info we just made
    let mut dir_hash_to_info_idx = ctx.new_dir_hash_to_info_idx(&dir_info);

    if dir_info.parent.as_u64() != 0x0 {
        match add_dir_info(ctx, path.parent().unwrap_or_else(|| panic!("Could not get parent of {:?}!", path))) {
            Ok(parent_dir_info) => {
                ctx.add_dir_info_to_parent(&parent_dir_info, &dir_hash_to_info_idx);
            
                // Since we just added a dirinfo, we need to update our new dir_hash_to_info_idx so when the parent/child structure is resolved, it doesnt end up pointing back to itself.
                dir_hash_to_info_idx.set_index(ctx.dir_infos_vec.len() as u32);
            },
            Err(err) => {
                match err {
                    DirectoryAdditionError::AlreadyExists(parent_dir_info) => {
                        // Since it already exists we can just add it to the parent without needing to do anything else
                        ctx.add_dir_info_to_parent(&parent_dir_info, &dir_hash_to_info_idx);
                    },
                    _ => {
                        return Err(err);
                    }
                }
            },
        }
    }

    // Make a new directory offset
    let new_dir_offset = ctx.new_directory_offset(&dir_info);
    
    // Update the dir_info's path index to be the length of the current amount of folder_offsets (to point to the right one)
    dir_info.path.set_index(ctx.folder_offsets_vec.len() as u32);

    ctx.push_dir_context(dir_info, dir_hash_to_info_idx, new_dir_offset);

    Ok(dir_info)
}

pub fn add_dir_info_with_base(ctx: &mut AdditionContext, path: &Path, base: &Path) -> Result<DirInfo, DirectoryAdditionError> {
    match add_dir_info(ctx, path) {
        Ok(_) | Err(DirectoryAdditionError::AlreadyExists(_)) => {
            let base_dir_info_path = FolderPathListEntry::from_path(base)?;
            let dir_info_path = FolderPathListEntry::from_path(path)?;

            let base_dir_info = *ctx.get_dir_info_from_hash_ctx(base_dir_info_path.path.hash40())?;
            let dir_info = ctx.get_dir_info_from_hash_ctx_mut(dir_info_path.path.hash40())?;

            dir_info.copy_from_source(&base_dir_info);

            Ok(*dir_info)
        },
        Err(err) => Err(err)
    }
}