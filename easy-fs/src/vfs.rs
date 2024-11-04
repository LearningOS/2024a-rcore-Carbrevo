use super::{
    block_cache_sync_all, get_block_cache, BlockDevice, DirEntry, DiskInode, DiskInodeType,
    EasyFileSystem, DIRENT_SZ,
};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::{Mutex, MutexGuard};
use crate::alloc::string::ToString;

/// Virtual filesystem layer over easy-fs
pub struct Inode {
    block_id: usize,
    block_offset: usize,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}

bitflags! {
    /// The mode of a inode
    /// whether a directory or a file
    pub struct StatMode: u32 {
        /// null
        const NULL  = 0;
        /// directory
        const DIR   = 0o040000;
        /// ordinary regular file
        const FILE  = 0o100000;
    }
}

impl Inode {
    /// Create a vfs inode
    pub fn new(
        block_id: u32,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            block_id: block_id as usize,
            block_offset,
            fs,
            block_device,
        }
    }
    /// Call a function over a disk inode to read it
    fn read_disk_inode<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .read(self.block_offset, f)
    }
    /// Call a function over a disk inode to modify it
    fn modify_disk_inode<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .modify(self.block_offset, f)
    }

    /// Find inode under a disk inode by name
    fn find_inode_id(&self, name: &str, disk_inode: &DiskInode) -> Option<u32> {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                DIRENT_SZ,
            );
            if dirent.name() == name {
                return Some(dirent.inode_id());
            }
        }
        None
    }

    /// Find inode under a disk inode by name
    fn find_dentry(&self, name: &str, disk_inode: &DiskInode) -> Option<(usize, Arc<DirEntry>)> {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                DIRENT_SZ,
            );
            if dirent.name() == name {
                return Some((i, Arc::new(dirent)));
            }
        }
        None
    }

    /// Find inode under a disk inode by name
    fn find_dent_pos(&self, name: &str, disk_inode: &DiskInode) -> Option<u32> {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                DIRENT_SZ,
            );
            if dirent.name() == name {
                return Some(i as u32)
            }
        }
        None
    }

    /// Find inode under a disk inode by name
    fn find_inode_by_id(&self, id: u32, disk_inode: &DiskInode) -> Vec<String> {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut names = Vec::<String>::new();
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                DIRENT_SZ,
            );    
            if dirent.inode_id() == id {
                names.push(dirent.name().to_string());
            }
        }
        names
    }

    /// Find inode under a disk inode by name
    fn find_ino_by_blkid_locked(&self, fs: &MutexGuard::<'_, EasyFileSystem>, (id, off): (u32, usize), disk_inode: &DiskInode) -> Option<u32> {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                DIRENT_SZ,
            );    
            let (block_id, block_offset) = fs.get_disk_inode_pos(dirent.inode_id());
            if block_id == id && block_offset == off {
                return Some(dirent.inode_id());
            }
        }
        None
    }

    ///
    pub fn node_id_locked(&self, fs: &MutexGuard::<'_, EasyFileSystem>) -> u32 {
        //let fs = self.fs.lock();
        fs.get_disk_inode_id((self.block_id as u32, self.block_offset))
    }

    ///
    pub fn node_id(&self) -> u32 {
        let fs = self.fs.lock();
        fs.get_disk_inode_id((self.block_id as u32, self.block_offset))
    }

    ///
    pub fn inode_id(&self) -> u32 {
        let root = EasyFileSystem::root_inode(&self.fs);
        let fs = self.fs.lock();
        let ino = root.read_disk_inode(|disk_inode| {
            self.find_ino_by_blkid_locked(&fs, (self.block_id as u32, self.block_offset), disk_inode).unwrap()
        });
        ino
    }

    ///
    pub fn mode(&self) -> StatMode {
        self.read_disk_inode(|diskinode|{
            if diskinode.is_dir() {
                StatMode::DIR
            } else if diskinode.is_file() {
                StatMode::FILE
            } else {
                StatMode::NULL
            }
        })
    }

    /// Find inode under current inode by name
    pub fn find(&self, name: &str) -> Option<Arc<Inode>> {
        let fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode).map(|inode_id| {
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id,
                    block_offset,
                    self.fs.clone(),
                    self.block_device.clone(),
                ))
            })
        })
    }

    /// Find inode under current inode by name
    pub fn find_locked(&self, name: &str, fs: &MutexGuard<EasyFileSystem>) -> Option<Arc<Inode>> {
        self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode).map(|inode_id| {
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id,
                    block_offset,
                    self.fs.clone(),
                    self.block_device.clone(),
                ))
            })
        })
    }

    /// Lookup dentry under current inode by name
    pub fn lookup_locked(&self, name: &str, fs: &MutexGuard<EasyFileSystem>) -> Option<(usize, Arc<DirEntry>)> {
        self.read_disk_inode(|disk_inode| {
            self.find_dentry(name, disk_inode)
        })
    }

    /// Replace dentry under current inode by name
    pub fn replace_locked(&self, name: &str, dent: &DirEntry, fs: &mut MutexGuard<EasyFileSystem>) -> Option<u32> {
        //let fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            self.find_dent_pos(name, disk_inode).map(|dent_idx| {
                self.write_at_locked(
                    dent_idx as usize * DIRENT_SZ,
                    dent.as_bytes(),
                    fs
                );
                dent_idx
            })
        })
    }

    fn show_dentries(&self) {
        let fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            assert!(disk_inode.is_dir());
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut dirent = DirEntry::empty();
            for i in 0..file_count {
                assert_eq!(
                    disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device),
                    DIRENT_SZ,
                );

                trace!(
                    "kernel:dir@{}[{}]: dentry={}/{}",
                    self.node_id_locked(&fs), i,
                    dirent.name(), dirent.inode_id(),
                );
            }
        });
    }

    /// Find inode under current inode by name
    pub fn find_by_id(&self, id: u32) -> Vec<String> {
        //self.show_dentries();
        let fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            self.find_inode_by_id(id, disk_inode)
        })
    }

    /// Find inode under current inode by name
    pub fn find_by_id_locked(&self, id: u32, fs: &MutexGuard<EasyFileSystem>) -> Vec<String> {
        //self.show_dentries();
        //let fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            self.find_inode_by_id(id, disk_inode)
        })
    }

    /// Increase the size of a disk inode
    fn increase_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    ) {
        if new_size < disk_inode.size {
            return;
        }
        let blocks_needed = disk_inode.blocks_num_needed(new_size);
        let mut v: Vec<u32> = Vec::new();
        for _ in 0..blocks_needed {
            v.push(fs.alloc_data());
        }
        disk_inode.increase_size(new_size, v, &self.block_device);
    }

    /// Increase the size of a disk inode
    fn decrease_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    ) {
        if new_size > disk_inode.size {
            panic!("decrease to invalid size");
        }
        let mut v: Vec<u32> = disk_inode.decrease_size(new_size, &self.block_device);
        for id in v.iter() {
            fs.dealloc_data(*id);
        }
    }

    /// Create inode under current inode by name
    pub fn create(&self, name: &str) -> Option<Arc<Inode>> {
        let mut fs = self.fs.lock();
        let op = |root_inode: &DiskInode| {
            // assert it is a directory
            assert!(root_inode.is_dir());
            // has the file been created?
            self.find_inode_id(name, root_inode)
        };
        if self.read_disk_inode(op).is_some() {
            return None;
        }
        // create a new file
        // alloc a inode with an indirect block
        let new_inode_id = fs.alloc_inode();
        // initialize inode
        let (new_inode_block_id, new_inode_block_offset) = fs.get_disk_inode_pos(new_inode_id);
        get_block_cache(new_inode_block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(new_inode_block_offset, |new_inode: &mut DiskInode| {
                new_inode.initialize(DiskInodeType::File);
            });
        self.modify_disk_inode(|root_inode| {
            // append file in the dirent
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // increase size
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // write dirent
            let dirent = DirEntry::new(name, new_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });

        let (block_id, block_offset) = fs.get_disk_inode_pos(new_inode_id);
        block_cache_sync_all();
        // return inode
        Some(Arc::new(Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        )))
        // release efs lock automatically by compiler
    }

    ///
    pub fn vfs_link(&self, old_name: &str, new_name: &str) -> isize {
        let mut fs = self.fs.lock();
        if let Some((_, src_dent)) = self.lookup_locked(old_name, &fs) {
            trace!("linking {}@{} to {}", old_name, src_dent.inode_id(), new_name);
            self.modify_disk_inode(|root_inode| {
                // append file in the dirent
                let file_count = (root_inode.size as usize) / DIRENT_SZ;
                let new_size = (file_count + 1) * DIRENT_SZ;
                // increase size
                self.increase_size(new_size as u32, root_inode, &mut fs);
                // write dirent
                let dirent = DirEntry::new(new_name, src_dent.inode_id());
                root_inode.write_at(file_count * DIRENT_SZ, dirent.as_bytes(), &self.block_device);
            });
    
            block_cache_sync_all();
            0
        } else {
            -1
        }
    }

    ///
    pub fn vfs_unlink(&self, name: &str) -> isize {
        let mut fs = self.fs.lock();
        trace!("111111111");
        if let Some((idx, dent)) = self.lookup_locked(name, &fs) {
            let names = self.find_by_id_locked(dent.inode_id(), &fs);
            let links = names.len();
            if links == 1 {
                let (block_id, block_offset) = fs.get_disk_inode_pos(dent.inode_id());
                let inode = Inode::new(
                                block_id,
                                block_offset,
                                self.fs.clone(),
                                self.block_device.clone(),
                            );
                trace!("222222222");
                inode.modify_disk_inode(|disk_inode| {
                    disk_inode.clear_size(&self.block_device);
                });                
                trace!("3333333333");
            }

            trace!("444444444444");
            self.modify_disk_inode(|root_inode| {
                // append file in the dirent
                let file_count = (root_inode.size as usize) / DIRENT_SZ;
                if idx < file_count -1 {
                    let mut last_dent = DirEntry::empty();
                    assert_eq!(
                        root_inode.read_at((file_count - 1) * DIRENT_SZ, last_dent.as_bytes_mut(), &self.block_device),
                        DIRENT_SZ,
                    );
                    assert_eq!(
                        root_inode.write_at(idx * DIRENT_SZ, last_dent.as_bytes(), &self.block_device),
                        DIRENT_SZ,
                    );
                }

                let new_size = (file_count - 1) * DIRENT_SZ;
                // increase size
                trace!("555555555555");
                self.decrease_size(new_size as u32, root_inode, &mut fs);
            });    
            trace!("6666666666");
        
            block_cache_sync_all();
            0
        } else {
            -1
        }
    }

    /// List inodes under current inode
    pub fn ls(&self) -> Vec<String> {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut v: Vec<String> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                v.push(String::from(dirent.name()));
            }
            v
        })
    }
    /// Read data from current inode
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| disk_inode.read_at(offset, buf, &self.block_device))
    }

    /// Read data from current inode
    pub fn read_at_locked(&self, offset: usize, buf: &mut [u8], fs: &MutexGuard<EasyFileSystem>) -> usize {
        //let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| disk_inode.read_at(offset, buf, &self.block_device))
    }

    /// Write data to current inode
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let mut fs = self.fs.lock();
        let size = self.modify_disk_inode(|disk_inode| {
            self.increase_size((offset + buf.len()) as u32, disk_inode, &mut fs);
            disk_inode.write_at(offset, buf, &self.block_device)
        });
        block_cache_sync_all();
        size
    }

    /// Write data to current inode
    pub fn write_at_locked(&self, offset: usize, buf: &[u8], fs: &mut MutexGuard<EasyFileSystem>) -> usize {
        let size = self.modify_disk_inode(|disk_inode| {
            self.increase_size((offset + buf.len()) as u32, disk_inode, fs);
            disk_inode.write_at(offset, buf, &self.block_device)
        });
        block_cache_sync_all();
        size
    }

    /// Clear the data in current inode
    pub fn clear(&self) {
        let mut fs = self.fs.lock();
        self.modify_disk_inode(|disk_inode| {
            let size = disk_inode.size;
            let data_blocks_dealloc = disk_inode.clear_size(&self.block_device);
            assert!(data_blocks_dealloc.len() == DiskInode::total_blocks(size) as usize);
            for data_block in data_blocks_dealloc.into_iter() {
                fs.dealloc_data(data_block);
            }
        });
        block_cache_sync_all();
    }
}
