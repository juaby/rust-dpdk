use std::mem;
use std::ffi::CString;
use std::os::raw::c_char;

use ffi::raw::*;

use errors::*;
use memzone::RteMemoryZone;

/// the structure for the memory configuration for the RTE.
pub struct RteMemoryConfig(*mut Struct_rte_mem_config);

impl RteMemoryConfig {
    fn from_ptr(cfg: *mut Struct_rte_mem_config) -> RteMemoryConfig {
        RteMemoryConfig(cfg)
    }

    /// Number of channels (0 if unknown).
    fn nchannel(&self) -> u32 {
        unsafe { (*self.0).nchannel }
    }

    /// Number of ranks (0 if unknown).
    fn nrank(&self) -> u32 {
        unsafe { (*self.0).nrank }
    }

    /// Memzone descriptors.
    fn memzones(&self) -> Vec<RteMemoryZone> {
        unsafe {
            Vec::from(&(*self.0).memzone[..(*self.0).memzone_cnt as usize])
                .iter()
                .map(|zone| RteMemoryZone::from_ptr(zone))
                .collect()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(i32)]
pub enum RteProcType {
    Auto = -1, // RTE_PROC_AUTO
    Primary = 0, // RTE_PROC_PRIMARY
    Secondary = 1, // RTE_PROC_SECONDARY
    Invalid = 2, // RTE_PROC_INVALID
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u32)]
pub enum RteLcoreRole {
    Rte = 0, // ROLE_RTE
    Off = 1, // ROLE_OFF
}

/// The global RTE configuration structure.
pub struct RteConfig(*mut Struct_rte_config);

impl RteConfig {
    fn from_ptr(cfg: *mut Struct_rte_config) -> RteConfig {
        RteConfig(cfg)
    }

    /// Id of the master lcore
    fn master_lcore(&self) -> u32 {
        unsafe { (*self.0).master_lcore }
    }

    /// Number of available logical cores.
    fn lcore_count(&self) -> usize {
        unsafe { (*self.0).lcore_count as usize }
    }

    /// Primary or secondary configuration
    fn process_type(&self) -> RteProcType {
        unsafe { mem::transmute((*self.0).process_type) }
    }

    /// State of cores.
    fn lcore_roles(&self) -> &'static [RteLcoreRole] {
        unsafe { mem::transmute(&(*self.0).lcore_role[..(*self.0).lcore_count as usize]) }
    }

    /// Memory configuration, which may be shared across multiple DPDK instances
    fn memory_config(&self) -> RteMemoryConfig {
        RteMemoryConfig::from_ptr(unsafe { (*self.0).mem_config })
    }
}

extern "C" {
    fn _rte_lcore_id() -> u32;
}

/// Initialize the Environment Abstraction Layer (EAL).
pub fn eal_init(args: &Vec<&str>) -> RteResult<usize> {
    let cstrs = args.iter().map(|&s| CString::new(s).unwrap());
    let mut ptrs: Vec<*mut c_char> = cstrs.map(|s| s.as_ptr() as *mut c_char).collect();

    let parsed = unsafe { rte_eal_init(args.len() as i32, ptrs.as_mut_ptr()) };

    if parsed < 0 {
        Err(RteError::Init)
    } else {
        Ok(parsed as usize)
    }
}

/// Function to terminate the application immediately,
/// printing an error message and returning the exit_code back to the shell.
pub fn eal_exit(code: i32, msg: &str) {
    unsafe {
        rte_exit(code, CString::new(msg).unwrap().as_ptr());
    }
}

/// Get the global configuration structure.
pub fn eal_config() -> RteConfig {
    unsafe { RteConfig::from_ptr(rte_eal_get_configuration()) }
}

/// Get the process type in a multi-process setup
pub fn process_type() -> RteProcType {
    unsafe { mem::transmute(rte_eal_process_type()) }
}

/// Get a lcore's role.
pub fn lcore_role(lcore_id: u32) -> RteLcoreRole {
    unsafe { mem::transmute(rte_eal_lcore_role(lcore_id)) }
}

/// Check if a primary process is currently alive
pub fn primary_proc_alive() -> bool {
    unsafe { rte_eal_primary_proc_alive(mem::zeroed()) != 0 }
}

/// Whether EAL is using huge pages (disabled by --no-huge option).
pub fn has_hugepages() -> bool {
    unsafe { rte_eal_has_hugepages() != 0 }
}

/// Return the ID of the execution unit we are running on.
pub fn lcore_id() -> u32 {
    unsafe { _rte_lcore_id() }
}

/// Get the id of the master lcore
pub fn master_lcore() -> u32 {
    eal_config().master_lcore()
}

/// Return the number of execution units (lcores) on the system.
pub fn lcore_count() -> usize {
    eal_config().lcore_count()
}

/// Return the ID of the physical socket of the logical core we are running on.
pub fn socket_id() -> u32 {
    unsafe { rte_socket_id() }
}

/// Get the ID of the physical socket of the specified lcore
pub fn lcore_to_socket_id(lcore_id: u32) -> u32 {
    unsafe { lcore_config[lcore_id as usize].socket_id }
}

#[cfg(test)]
mod tests {
    extern crate num_cpus;

    use super::*;

    #[test]
    fn test_eal() {
        assert_eq!(eal_init(&vec!["-n 2", "-r 4"]).unwrap(), 0);

        assert_eq!(process_type(), RteProcType::Primary);
        assert!(!primary_proc_alive());
        assert!(has_hugepages());
        assert_eq!(lcore_role(lcore_id()), RteLcoreRole::Rte);
        assert_eq!(lcore_id(), 0);
        assert_eq!(master_lcore(), 0);
        assert_eq!(lcore_count(), num_cpus::get());
        assert_eq!(socket_id(), 0);
        assert_eq!(lcore_to_socket_id(lcore_id()), 0);

        let eal_cfg = eal_config();

        assert_eq!(eal_cfg.master_lcore(), 0);
        assert_eq!(eal_cfg.lcore_count(), num_cpus::get());
        assert_eq!(eal_cfg.process_type(), RteProcType::Primary);
        assert_eq!(eal_cfg.lcore_roles(),
                   &[RteLcoreRole::Rte, RteLcoreRole::Rte, RteLcoreRole::Rte, RteLcoreRole::Rte]);

        let mem_cfg = eal_cfg.memory_config();

        assert_eq!(mem_cfg.nchannel(), 2);
        assert_eq!(mem_cfg.nrank(), 4);

        let memzones = mem_cfg.memzones();

        assert_eq!(memzones.len(), 2);
    }
}
