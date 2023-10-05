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
pub enum FileAdditionError {
    #[error("path has no parent")]
    MissingParent,
    #[error("hashing error")]
    Hash(#[from] HashingError),
    #[error("lookup error")]
    Lookup(#[from] LookupError),
    #[error("directory addition error")]
    Directory(#[from] DirectoryAdditionError) // lol, lmao
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

fn add_searchable_folder_by_folder(ctx: &mut SearchContext, folder: &Folder) -> bool {
    // begin by simply checking if this folder's parent exists
    // eventually up the chain we should be able to find an existing folder to add our tree into
    let Some(parent) = folder.parent.as_ref() else {
        error!("Cannot add folder recursively because it has no parent");
        return false;
    };

    // do a similar check to the file to see if the parent exists, and if not then we want to add it
    let has_parent = ctx.get_folder_path_mut(parent.full_path.to_smash_arc()).is_some();

    // if we can't find or add anything, just jump out and let the user die
    if !has_parent && !add_searchable_folder_by_folder(ctx, parent) {
        error!("Cannot add folder recursively because we failed to find/add its parent");
        return false;
    }

    // quick check on the fields of folder to ensure that we can actually do this
    if folder.name.is_none() {
        error!("Cannot add folder with no name");
        return false;
    }

    let path_list_indices_len = ctx.path_list_indices.len();

    // get the parent, we can't *really* fail here, and if we do then something is broken in the ctx impl
    let Some(parent) = ctx.get_folder_path_mut(parent.full_path.to_smash_arc()) else {
        error!("Failed to get parent after ensuring that it exists");
        return false;
    };

    let mut new_folder = FolderPathListEntry::from_folder(folder);
    // Create a new directory that does not have child directories
    new_folder.set_first_child_index(0xFF_FFFF);
    // Create a new search path
    let mut new_path = new_folder.as_path_entry();
    // Set the previous head of the linked list as the child of the new path
    new_path.path.set_index(parent.get_first_child_index() as u32);
    // Set the next path as the first element of the linked list
    parent.set_first_child_index(path_list_indices_len as u32);
    ctx.new_folder_paths.insert(new_folder.path.hash40(), ctx.folder_paths.len());
    ctx.new_paths.insert(new_path.path.hash40(), ctx.path_list_indices.len());
    ctx.path_list_indices.push(ctx.paths.len() as u32);
    ctx.folder_paths.push(new_folder);
    ctx.paths.push(new_path);

    true
}

pub fn add_shared_searchable_file(ctx: &mut SearchContext, new_file: &File) {
    // yes this is complex
    // yes I apologize
    // but for this feature to be complete it should be able to add new files which don't have parents
    // in a search folder
    // I either do it now while I'm thinking about it or never do it at all

    // first, we try and just get the raw parent
    // we have to evaluate this into a boolean because of references
    // I am working on a better file addition implementation that doesn't have this dogshit
    // workaround but it will take some time
    let has_parent = ctx.get_folder_path_mut(new_file.parent.full_path.to_smash_arc()).is_some();

    // if it isn't there, then we are going to recursively add it's parent, returning out if it's not possible
    if !has_parent && !add_searchable_folder_by_folder(ctx, &new_file.parent) {
        error!("Cannot add shared file to search section because we could not add it's parents");
        return;
    }

    // we get the length here because we are about to acquire a mutable reference
    // to the parent folder, and we cannot get the length via immutable reference
    // if that one is active
    let path_list_indices_len = ctx.path_list_indices.len();

    let Some(parent) = ctx.get_folder_path_mut(new_file.parent.full_path.to_smash_arc()) else {
        error!("Cannot add shared file to search section because its parent does not exist");
        return;
    };

    let mut new_file = PathListEntry::from_file(new_file);

    new_file.path.set_index(parent.get_first_child_index() as u32);
    // Set the file as the head of the linked list
    parent.set_first_child_index(path_list_indices_len as u32);
    ctx.new_paths.insert(new_file.path.hash40(), ctx.path_list_indices.len());
    ctx.path_list_indices.push(ctx.paths.len() as u32);
    ctx.paths.push(new_file);
}

pub fn add_searchable_file_recursive(ctx: &mut SearchContext, path: &Path) -> Result<(), FileAdditionError> {
    let parent = path.parent().unwrap_or_else(|| panic!("Failed to get the parent for path '{}'", path.display()));

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
                .ok_or(LookupError::Missing)?
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

pub fn add_files_to_directory(ctx: &mut AdditionContext, directory: Hash40, files: HashSet<Hash40>) -> Result<(), LookupError> {
    // Get the file info range of the directory that was passed in
    let file_info_range = ctx.get_dir_info_from_hash_ctx(directory).map(|dir| dir.file_info_range())?;

    // Create new Vec with the size of the file count of the specified dir + the count of the files passed in
    let mut file_infos = Vec::with_capacity(file_info_range.len() + files.len());

    // Create new HashSet for the current files in the specified dir
    let mut contained_files = HashSet::new();

    // Loop through all files in the file_infos at the directory position
    for file_info in ctx.file_infos[file_info_range.clone()].iter() {
        // Insert directory file hash40 to the contained files set
        contained_files.insert(ctx.filepaths[usize::from(file_info.file_path_index)].path.hash40());

        // If the file_info_range has an entry for the FileInfoIndex for the current file, then update the FileInfoIdx
        // with a new FileInfoIdx that takes the context file_infos length + the length of the current file_infos (don't know why this is done)
        if file_info_range.contains(&usize::from(ctx.get_file_info_idx_by_filepath_idx(file_info.file_path_index))) {
            let fileinfoindice_idx = ctx.get_filepath_by_idx(file_info.file_path_index).path.index() as usize;
            // Can't use the util method here because it's being indexed ... Needs a get/get_mut or something
            ctx.file_info_indices[fileinfoindice_idx].file_info_index = FileInfoIdx((ctx.file_infos.len() + file_infos.len()) as u32);
        }

        // Add file_info to the file_infos vector created earlier
        file_infos.push(*file_info);
    }

    for file in files {
        // If the file passed in is already exists, then just skip it
        if contained_files.contains(&file) {
            continue;
        }

        // Get the FilePathIdx from the context
        if let Some(file_index) = ctx.get_path_index_from_hash(file) {
            // Get the FileInfo from the context FileInfos with the FileInfoIndex with the file_index gotten
            // earlier
            let mut file_info = ctx.file_infos[usize::from(ctx.get_file_info_idx_by_filepath_idx(file_index))];

            // only change the file linkage/file datas if we aren't a new shared file
            // changing those things has unintended/cataclysmic behavior lmfao
            if !file_info.flags.new_shared_file() {
                let fileinfo_idx = usize::from(ctx.get_file_info_idx_by_filepath_idx(file_index));
                // Get the FileInfoToData from the InfoToData array context
                let info_to_data = &mut ctx.info_to_datas[usize::from(ctx.file_infos[fileinfo_idx].info_to_data_index)];

                // Set the folder offset index to 0
                info_to_data.folder_offset_index = 0x0;

                // Get the data index from the info -> data
                let data_idx = info_to_data.file_data_index;

                // Get the FileData from the FileDatas with the FileDataIdx gotten earlier
                let file_data = &mut ctx.file_datas[usize::from(data_idx)];

                // Set the compressed and decompressed size to 0x100 (256) (The decompressed size will change later
                // when patched by ARCropolis)
                file_data.comp_size = 0x100;
                file_data.decomp_size = 0x100;

                // Set the FileData offset in folder to 0 so it at least has a value
                file_data.offset_in_folder = 0x0;

                // Set the flags to not be compressed and not use zstd
                file_data.flags = FileDataFlags::new().with_compressed(false).with_use_zstd(false);
            }

            file_info.file_path_index = file_index;
            file_info.flags.set_standalone_file(true);

            // Set the file info index to the current context file infos size + the current length of the
            // file_infos vector created earlier
            if !file_info.flags.new_shared_file() {
                let file_info_indice_idx = ctx.get_filepath_by_idx(file_index).path.index() as usize;

                ctx.file_info_indices[file_info_indice_idx].file_info_index =
                    FileInfoIdx((ctx.file_infos.len() + file_infos.len()) as u32);
            }

            // Push the modified file_info to the file_infos vector
            file_infos.push(file_info);
        } else {
            error!("Cannot get file path index for '{}' ({:#x})", hashes::find(file), file.0);
        }
    }

    // Get the new start index by getting the length of the context file_infos (so we're changing the start
    // position of the directory to be at the end of the old file_infos)
    let file_start_index = ctx.file_infos.len() as u32;

    // Take our newly generated file_infos and append it to the context file_infos
    ctx.file_infos.extend_from_slice(&file_infos);

    // Get the directory from the context
    let dir_info = ctx
        .get_dir_info_from_hash_ctx_mut(directory)
        .expect("Failed to get directory after confirming it exists");

    // Modify the directory start index and the file count
    dir_info.file_info_start_index = file_start_index;
    dir_info.file_count = file_infos.len() as u32;
    // info!("Added files to {} ({:#x})", hashes::find(directory), directory.0);

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