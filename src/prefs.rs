use crate::error::Result;

pub fn path_opted_out() -> Result<bool> {
    Ok(crate::options::load()?.path_optout)
}

pub fn set_path_opted_out(opt_out: bool) -> Result<()> {
    crate::options::set_path_optout(opt_out)
}
