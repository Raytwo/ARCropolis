use std::{collections::HashSet, path::Path};

use arc_config::{
    search::{File, Folder},
    ToSmashArc,
};
use smash_arc::*;

use super::{
    lookup,
    AdditionContext,
    FromPathExt,
    FromSearchableFile,
    FromSearchableFolder,
    SearchContext
};
use crate::{
    hashes,
    replacement::FileInfoFlagsExt,
    resource::{LoadedData, LoadedFilepath},
    PathExtension,
    resource::CppVector
};

pub fn add_file(ctx: &mut AdditionContext, path: &Path) {
    // Create a FilePath from the path that's passed in
    let mut file_path = if let Some(file_path) = FilePath::from_path(path) {
        file_path
    } else {
        error!("Failed to generate a FilePath from {}!", path.display());
        return;
    };

    // Create a new FilePathIdx by getting the length of all filepaths in the vector
    let filepath_idx = FilePathIdx(ctx.filepaths.len() as u32);
    // Create a new FileInfoIndiceIdx by getting the length of all file_info_indices in the vector
    let file_info_indice_idx = FileInfoIndiceIdx(ctx.file_info_indices.len() as u32);
    // Create a new FileInfoIdx by getting the length of all file_infos in the vector
    let file_info_idx = FileInfoIdx(ctx.file_infos.len() as u32);
    // Create a new InfoToDataIdx by getting the length of all info_to_datas in the vector
    let info_to_data_idx = InfoToDataIdx(ctx.info_to_datas.len() as u32);
    // Create a new FileDataIdx by getting the length of all file_datas in the vector
    let file_data_idx = FileDataIdx(ctx.file_datas.len() as u32);

    // Create a base file for the new file from mario's numdlb and set the region to none
    let base_file = ctx.get_file_in_folder(
        ctx.get_file_info_from_hash(Hash40::from("fighter/mario/model/body/c00/model.numdlb"))
            .unwrap(),
        Region::None,
    );

    // Create a new FileInfoIndex with the created file_info_idx above and a dir offset index of
    let new_info_indice_idx = FileInfoIndex {
        dir_offset_index: 0xFF_FFFF,
        file_info_index: file_info_idx,
    };

    // Create a new FileInfo with the information created above and set unknown1 flags true if file is either
    // a nutexb or eff
    let mut new_file_info = FileInfo {
        file_path_index: filepath_idx,
        file_info_indice_index: file_info_indice_idx,
        info_to_data_index: info_to_data_idx,
        flags: FileInfoFlags::new().with_unknown1(file_path.ext.hash40() == Hash40::from("nutexb") || file_path.ext.hash40() == Hash40::from("eff")),
    };

    // Set the new file to be standalone so it doesn't need to be near the other files
    new_file_info.flags.set_standalone_file(true);

    // Create new FileInfoToData with the folder_offset_index of the base file from earlier,
    // file_data_index from the newly generated file_data_idx above, and a file info index and load type of 1
    let new_info_to_data = FileInfoToFileData {
        folder_offset_index: base_file.folder_offset_index,
        file_data_index: file_data_idx,
        file_info_index_and_load_type: FileInfoToFileDataBitfield::new().with_load_type(1),
    };

    // Redeclare base_file with the file data from fhe file data index of the base file
    let base_file = ctx.get_file_datas()[usize::from(base_file.file_data_index)];

    // Create a new FileData with the base_file FileData information
    let new_file_data = FileData {
        offset_in_folder: base_file.offset_in_folder,
        comp_size: base_file.comp_size,
        decomp_size: base_file.decomp_size,
        flags: FileDataFlags::new().with_compressed(false).with_use_zstd(false),
    };

    // Set the FilePath's path index to be the new file_info_indice_idx created earlier
    file_path.path.set_index(file_info_indice_idx.0);

    // Push all the newly created variables to the appropiate context vectors
    ctx.filepaths.push(file_path);
    ctx.file_info_indices.push(new_info_indice_idx);
    ctx.file_infos.push(new_file_info);
    ctx.info_to_datas.push(new_info_to_data);
    ctx.file_datas.push(new_file_data);

    // Push default values to the loaded_(filepaths/datas) to make it match up with the other vectors length
    ctx.loaded_filepaths.push(LoadedFilepath::default());
    ctx.loaded_datas.push(LoadedData::new());

    // Insert the added FilePath's path and it's index to the context's added_files vector
    ctx.added_files.insert(file_path.path.hash40(), filepath_idx);

    info!("Added file '{}' ({:#x})", path.display(), file_path.path.hash40().0);
}

pub fn add_shared_file(ctx: &mut AdditionContext, new_file: &File, shared_to: Hash40) {
    // Get the target shared FileInfoIndice index
    let info_indice_idx = if let Ok(info) = ctx.get_file_info_from_hash(shared_to) {
        info.file_info_indice_index.0
    } else if let Some(file_path_idx) = ctx.added_files.get(&shared_to) {
        let info_index = ctx.filepaths[usize::from(*file_path_idx)].path.index() as usize;
        let info_idx = ctx.file_info_indices[info_index].file_info_index;
        ctx.file_infos[usize::from(info_idx)].file_info_indice_index.0
    } else {
        error!(
            "Failed to find file '{}' ({:#x}) when attempting to share file to it.",
            hashes::find(shared_to),
            shared_to.0
        );
        return;
    };

    // Make FilePath from path passed in
    let mut filepath = FilePath::from_file(new_file);

    // Set the FilePath's path index to the shared target FileInfoIndice index
    filepath.path.set_index(info_indice_idx);

    // Push the FilePath to the context FilePaths
    ctx.filepaths.push(filepath);
    ctx.added_files
        .insert(filepath.path.hash40(), FilePathIdx((ctx.filepaths.len() - 1) as u32));
    ctx.loaded_filepaths.push(LoadedFilepath::default());

    // Add the shared file to the lookup
    lookup::add_shared_file(
        new_file.full_path.to_smash_arc(), // we can unwrap because of FilePath::from_path being successful
        shared_to.to_smash_arc(),
    );
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
        },
        Some(parent) => match parent.smash_hash() {
            Ok(hash) => {
                let len = ctx.path_list_indices.len();
                match ctx.get_folder_path_mut(hash) {
                    Some(parent) => (parent, len),
                    None => {
                        add_searchable_folder_recursive(ctx, parent);
                        let len = ctx.path_list_indices.len();
                        match ctx.get_folder_path_mut(hash) {
                            Some(parent) => (parent, len),
                            None => {
                                error!("Unable to add folder '{}'", parent.display());
                                return;
                            },
                        }
                    },
                }
            },
            Err(e) => {
                error!("Unable to get the smash hash for '{}'. {:?}", parent.display(), e);
                return;
            },
        },
        None => {
            error!("Failed to get the parent for path '{}'", path.display());
            return;
        },
    };

    if let Some(mut new_folder) = FolderPathListEntry::from_path(path) {
        // Create a new directory that does not have child directories
        new_folder.set_first_child_index(0xFF_FFFF);
        // Create a new search path
        let mut new_path = new_folder.as_path_entry();
        // Set the previous head of the linked list as the child of the new path
        new_path.path.set_index(parent.get_first_child_index() as u32);
        // Set the next path as the first element of the linked list
        parent.set_first_child_index(current_path_list_indices_len as u32);
        ctx.new_folder_paths.insert(new_folder.path.hash40(), ctx.folder_paths.len());
        ctx.new_paths.insert(new_path.path.hash40(), ctx.path_list_indices.len());
        ctx.path_list_indices.push(ctx.paths.len() as u32);
        ctx.folder_paths.push(new_folder);
        ctx.paths.push(new_path);
    } else {
        error!("Failed to add folder {}!", path.display());
    }
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

pub fn add_searchable_file_recursive(ctx: &mut SearchContext, path: &Path) {
    // Create a parent and current_path_list_indices_len variable tuple
    let (parent, current_path_list_indices_len) = match path.parent() {
        // If the parent is empty, then just return
        Some(parent) if parent == Path::new("") => {
            error!("Cannot add file {} as root file!", path.display());
            return;
        },
        // Else if the parent is alright (actually something), keep going
        Some(parent) => {
            // Convert the parent to a smash_hash
            match parent.smash_hash() {
                // If it gets the hash, do the following
                Ok(hash) => {
                    // Get the length of the current path list indices
                    let len = ctx.path_list_indices.len();
                    // Try getting the folder path from the context
                    match ctx.get_folder_path_mut(hash) {
                        // If the folder path exists, then just return the parent and the length
                        Some(parent) => (parent, len),
                        None => {
                            // If it doesn't exist, then call the add_searchable_folder_recursive
                            // with the parent path
                            add_searchable_folder_recursive(ctx, parent);
                            // Get the new length since it changed thanks to the previous function call
                            let len = ctx.path_list_indices.len();
                            // Try getting the folder path again
                            match ctx.get_folder_path_mut(hash) {
                                // If the folder now exists, return the parent and length
                                Some(parent) => (parent, len),
                                // Else, just return
                                None => {
                                    error!("Unable to add folder '{}'", parent.display());
                                    return;
                                },
                            }
                        },
                    }
                },
                Err(e) => {
                    error!("Unable to get the smash hash for '{}'. {:?}", parent.display(), e);
                    return;
                },
            }
        },
        None => {
            error!("Failed to get the parent for path '{}'", path.display());
            return;
        },
    };

    // Try getting the file from the path after adding the folders
    if let Some(mut new_file) = PathListEntry::from_path(path) {
        // info!(
        //     "Adding file '{}' ({:#}) to folder '{}' ({:#x})",
        //     hashes::find(new_file.path.hash40()),
        //     new_file.path.hash40().0,
        //     hashes::find(parent.path.hash40()),
        //     parent.path.hash40().0
        // );
        // Set the previous head of the linked list as the child of the new file
        new_file.path.set_index(parent.get_first_child_index() as u32);
        // Set the file as the head of the linked list
        parent.set_first_child_index(current_path_list_indices_len as u32);
        ctx.new_paths.insert(new_file.path.hash40(), ctx.path_list_indices.len());
        ctx.path_list_indices.push(ctx.paths.len() as u32);
        ctx.paths.push(new_file);
    } else {
        error!("Failed to add folder {}!", path.display());
    }
}

pub fn add_files_to_directory(ctx: &mut AdditionContext, directory: Hash40, files: HashSet<Hash40>) {
    fn get_path_idx(ctx: &AdditionContext, hash: Hash40) -> Option<FilePathIdx> {
        // Try getting the FilePathIdx from the passed in hash, if failed, then get index from the added files
        match ctx.get_file_path_index_from_hash(hash) {
            Ok(idx) => Some(idx),
            Err(_) => ctx.added_files.get(&hash).copied(),
        }
    }

    // Get the file info range of the directory (dirinfo?) that was passed in
    let file_info_range = match ctx.get_dir_info_from_hash_ctx(directory) {
        Ok(dir) => dir.file_info_range(),
        Err(_) => {
            error!("Cannot get file info range for '{}' ({:#x})", hashes::find(directory), directory.0);
            return;
        },
    };

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
        if file_info_range.contains(&usize::from(
            ctx.file_info_indices[ctx.filepaths[usize::from(file_info.file_path_index)].path.index() as usize].file_info_index,
        )) {
            ctx.file_info_indices[ctx.filepaths[usize::from(file_info.file_path_index)].path.index() as usize].file_info_index =
                FileInfoIdx((ctx.file_infos.len() + file_infos.len()) as u32);
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
        if let Some(file_index) = get_path_idx(ctx, file) {
            // Get the FileInfoToData from the InfoToData array context
            let info_to_data = &mut ctx.info_to_datas[usize::from(
                ctx.file_infos[usize::from(ctx.file_info_indices[ctx.filepaths[usize::from(file_index)].path.index() as usize].file_info_index)]
                    .info_to_data_index,
            )];

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

            // Get the FileInfo from the context FileInfos with the FileInfoIndex with the file_index gotten
            // earlier
            let file_info =
                ctx.file_infos[usize::from(ctx.file_info_indices[ctx.filepaths[usize::from(file_index)].path.index() as usize].file_info_index)];

            // Set the file info index to the current context file infos size + the current length of the
            // file_infos vector created earlier
            ctx.file_info_indices[ctx.filepaths[usize::from(file_index)].path.index() as usize].file_info_index =
                FileInfoIdx((ctx.file_infos.len() + file_infos.len()) as u32);

            // Push the modified file_info to the file_infos vector
            file_infos.push(file_info);
        } else {
            error!("Cannot get file path index for '{}' ({:#x})", hashes::find(file), file.0);
        }
    }

    // Get the new start index by getting the length of the context file_infos (so we're changing the start
    // position of the directory to be at the end of the old file_infos)
    let start_index = ctx.file_infos.len() as u32;

    // Take our newly generated file_infos and append it to the context file_infos
    ctx.file_infos.extend_from_slice(&file_infos);

    // Get the directory from the context
    let dir_info = ctx
        .get_dir_info_from_hash_ctx_mut(directory)
        .expect("Failed to get directory after confirming it exists");

    // Modify the directory start index and the file count
    dir_info.file_info_start_index = start_index;
    dir_info.file_count = file_infos.len() as u32;
    // info!("Added files to {} ({:#x})", hashes::find(directory), directory.0);
}

// Right now this will take up a bit of memory if adding multiple dirs to the same dirinfo, so gonna have to change it to take a vec instead ig
pub fn add_dir_info_to_parent(ctx: &mut AdditionContext, parent_dir_info: &mut DirInfo, child_hash_to_index: &HashToIndex){
    let mut parent_folder_children_hashes = ctx.folder_children_hashes[parent_dir_info.children_range()]
                                                .iter()
                                                .map(|child| child.clone())
                                                .collect::<Vec<_>>();

    // Change so that it equals len of vector if this ever gets changed
    parent_dir_info.child_dir_count += 1;
    parent_dir_info.child_dir_start_index = ctx.folder_children_hashes.len() as u32;
        
    // Add new child hash to the index
    parent_folder_children_hashes.push(child_hash_to_index.clone());
    ctx.folder_children_hashes.extend_from_slice(&parent_folder_children_hashes[..]);
}

pub fn add_dir_info(ctx: &mut AdditionContext, path: &Path) {
    // Create a FolderPathListEntry from the path that's passed in
    let dir_info_path = if let Some(dir_info_path) = FolderPathListEntry::from_path(path) {
        dir_info_path
    } else {
        error!("Failed to generate a FolderPathListEntry from {}!", path.display());
        return
    };

    // Get a base
    let mut dir_info = ctx.get_dir_info_from_hash_ctx(Hash40::from("fighter/luigi/c00")).unwrap().clone();

    let dir_hash_to_info_idx = HashToIndex::new()
                                                .with_hash(dir_info_path.path.hash())
                                                .with_length(dir_info_path.path.length())
                                                .with_index(ctx.dir_infos_vec.len() as u32);

    // Set dir_info values to our new dir_info info
    dir_info.path = dir_info_path.path;
    dir_info.name = dir_info_path.file_name.hash40();
    dir_info.parent = dir_info_path.parent.hash40();
    dir_info.file_info_start_index = 0;
    dir_info.file_count = 0;
    dir_info.child_dir_start_index = 0;
    dir_info.child_dir_count = 0;
    // dir_info.flags =  DirInfoFlags::new().with_unk1(0).with_redirected(false).with_unk2(false).with_is_symlink(false).with_unk3(0);



    // --------------------- FOLDER CHILD HASHES DONE HERE --------------------- // 
    // Check to see if parent actually exists
    if dir_info_path.parent.hash40().as_u64() != 0x0 {
        // If so, try getting the parent dir info
        match ctx.get_dir_info_from_hash_ctx(dir_info_path.parent.hash40()) {
            // If successful, add the current dir info the parent
            Ok(parent_dir_info) => {
                // Clone the parent dir info so we can make it mutable
                let mut parent_dir_info_mut = parent_dir_info.clone();
                add_dir_info_to_parent(ctx, &mut parent_dir_info_mut, &dir_hash_to_info_idx);

                // We can unwrap directly because if we're here, the parent does exist
                *ctx.get_dir_info_from_hash_ctx_mut(dir_info_path.parent.hash40()).unwrap() = parent_dir_info_mut;
            },
            // Else, just say you failed at getting the parent dirinfo and say why
            Err(_err) => {
                match path.parent() {
                    // If a parent does exist in the path but parent doesn't exist in the DirInfos,
                    // add it
                    Some(parent) => {
                        add_dir_info(ctx, &parent);
                        // After adding it, go ahead and try the logic from above again
                        match ctx.get_dir_info_from_hash_ctx(dir_info_path.parent.hash40()) {
                            Ok(parent_dir_info) => {
                                let mut parent_dir_info_mut = parent_dir_info.clone();
                                add_dir_info_to_parent(ctx, &mut parent_dir_info_mut, &dir_hash_to_info_idx);
                                *ctx.get_dir_info_from_hash_ctx_mut(dir_info_path.parent.hash40()).unwrap() = parent_dir_info_mut;
                            },
                            Err(err) => {
                                println!("Failed getting DirInfo Parent ({:#x})! Reason: {:?}", dir_info_path.parent.hash40().as_u64(), err);
                            }
                        }
                    },
                    None => {
                        println!("Could not get parent of {:?}!", path);
                        return;
                    }
                }
            }
        }
    }
    // --------------------- END FOLDER CHILD HASHES --------------------- // 
    
    
    // --------------------- FOLDER OFFSETS DONE HERE (FIGURE OUT STUFF ABOUT THIS LATER IF IT DOESN'T WORK) --------------------- //
    let new_dir_offset = DirectoryOffset {
        offset: 0,
        decomp_size: 0,
        size: 0,
        file_start_index: dir_info.file_info_start_index,
        file_count: dir_info.file_count,
        directory_index: dir_hash_to_info_idx.index(),
    };
    
    dir_info.path.set_index(ctx.folder_offsets_vec.len() as u32);
    // --------------------- END FOLDER OFFSETS --------------------- //
    
    
    // --------------------- PUSH TO CONTEXT DONE HERE --------------------- //
    ctx.dir_infos_vec.push(dir_info);
    ctx.dir_hash_to_info_idx.push(dir_hash_to_info_idx);
    ctx.folder_offsets_vec.push(new_dir_offset);

    let mut dir_hash_to_info_index_sorted = ctx.dir_hash_to_info_idx.iter().map(|idx| idx.clone()).collect::<Vec<_>>();
    
    dir_hash_to_info_index_sorted.sort_by(|a, b| {
        a.hash40().cmp(&b.hash40())
    });
    
    ctx.dir_hash_to_info_idx = CppVector::from_slice(&dir_hash_to_info_index_sorted[..]);
    // --------------------- END PUSH TO CONTEXT --------------------- //
}

pub fn add_dir_info_with_base(ctx: &mut AdditionContext, path: &Path, base: &Path) {
    // Create a FolderPathListEntry from the path that's passed in
    let dir_info_path = if let Some(dir_info_path) = FolderPathListEntry::from_path(path) {
        dir_info_path
    } else {
        error!("Failed to generate a FolderPathListEntry from {} for dir_info_path!", path.display());
        return
    };
    
    // Create a FolderPathListEntry from the path that's passed in
    let base_dir_info_path = if let Some(base_dir_info_path) = FolderPathListEntry::from_path(base) {
        base_dir_info_path
    } else {
        error!("Failed to generate a FolderPathListEntry from {} for base_dir_info_path!", base.display());
        return
    };

    add_dir_info(ctx, path);

    // Get the base
    let base_dir_info = ctx.get_dir_info_from_hash_ctx(base_dir_info_path.path.hash40()).unwrap().clone();

    // Get the newly added dirinfo
    let mut dir_info = ctx.get_dir_info_from_hash_ctx_mut(dir_info_path.path.hash40()).unwrap();


    // Set dir_info values to the base dirinfo
    dir_info.path.set_index(base_dir_info.path.index());
    dir_info.file_info_start_index = base_dir_info.file_info_start_index;
    dir_info.file_count = base_dir_info.file_count;
    dir_info.child_dir_start_index = base_dir_info.child_dir_start_index;
    dir_info.child_dir_count = base_dir_info.child_dir_count;
    dir_info.flags =  base_dir_info.flags;
}