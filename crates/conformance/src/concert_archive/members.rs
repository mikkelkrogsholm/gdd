//! Read regular TAR members with GNU long names and length-delimited PAX metadata.
use anyhow::{Context, Result, bail, ensure};
use std::collections::HashSet;
use std::io::{self, Read};
use tar::Header;

#[derive(Default)]
struct Pax {
    path: Option<Vec<u8>>,
    size: Option<u64>,
}

fn pax_fields(mut bytes: &[u8]) -> Result<Pax> {
    let mut fields = Pax::default();
    let mut keys = HashSet::new();
    while !bytes.is_empty() {
        let space = bytes
            .iter()
            .position(|&b| b == b' ')
            .context("Missing PAX record length")?;
        let digits = &bytes[..space];
        ensure!(
            digits.iter().all(u8::is_ascii_digit),
            "Invalid PAX record length"
        );
        let length: usize = std::str::from_utf8(digits)?.parse()?;
        let record = bytes
            .get(space + 1..length)
            .and_then(|s| s.strip_suffix(b"\n"))
            .context("Invalid PAX record length or terminator")?;
        let equals = record
            .iter()
            .position(|&b| b == b'=')
            .context("Missing PAX field value")?;
        let key = std::str::from_utf8(&record[..equals])?;
        ensure!(
            !key.is_empty() && keys.insert(key),
            "Empty or duplicate PAX key: {key}"
        );
        let value = &record[equals + 1..];
        match key {
            "path" => fields.path = Some(value.to_vec()),
            "size" => fields.size = Some(std::str::from_utf8(value)?.parse()?),
            "linkpath" => bail!("Linked archive member"),
            "SCHILY.filetype" => bail!("Unsupported PAX file type"),
            k if k.starts_with("GNU.sparse.") => bail!("Unsupported PAX file type"),
            _ => {}
        }
        bytes = &bytes[length..];
    }
    Ok(fields)
}

// tar's raw iterator validates a single 512-byte header and its checksum.
// Reading the body ourselves lets a PAX size override the USTAR size, even
// when a PAX path contains a newline that tar's line-based parser cannot read.
fn next_header(reader: &mut impl Read) -> Result<Option<Header>> {
    let mut archive = tar::Archive::new(reader);
    Ok(archive
        .entries()?
        .raw(true)
        .next()
        .transpose()?
        .map(|e| e.header().clone()))
}

pub(super) fn regular_files(
    reader: &mut impl Read,
    git_archive: bool,
    mut visit: impl FnMut(String, u32, u64, &mut dyn Read) -> Result<()>,
) -> Result<()> {
    let mut pax: Option<Pax> = None;
    let mut long_name = None;
    while let Some(header) = next_header(reader)? {
        let kind = header.entry_type();
        let extended = header.as_gnu().is_some() || header.as_ustar().is_some();
        let size = if kind.is_file() || kind.is_dir() {
            pax.as_ref().and_then(|p| p.size)
        } else {
            None
        }
        .unwrap_or(header.entry_size()?);

        let mut body = reader.by_ref().take(size);
        if extended && kind.is_pax_local_extensions() {
            ensure!(pax.is_none(), "Duplicate PAX extension header");
            let mut bytes = Vec::new();
            body.read_to_end(&mut bytes)?;
            pax = Some(pax_fields(&bytes)?);
        } else if extended && kind.is_gnu_longname() {
            ensure!(long_name.is_none(), "Duplicate GNU long name");
            let mut bytes = Vec::new();
            body.read_to_end(&mut bytes)?;
            if bytes.last() == Some(&0) {
                bytes.pop();
            }
            long_name = Some(bytes);
        } else if git_archive && kind.is_pax_global_extensions() {
            // Git stores the commit ID as a global PAX comment.
            io::copy(&mut body, &mut io::sink())?;
        } else {
            let name = long_name
                .take()
                .or_else(|| pax.as_mut().and_then(|p| p.path.take()));
            pax = None;
            if git_archive && kind.is_dir() {
                io::copy(&mut body, &mut io::sink())?;
            } else {
                ensure!(
                    kind.is_file() && header.link_name_bytes().is_none(),
                    "Expected a regular unlinked archive member"
                );
                let name = name.unwrap_or_else(|| header.path_bytes().into_owned());
                let name = String::from_utf8(name).context("Archive path is not UTF-8")?;
                visit(name, header.mode()?, size, &mut body)?;
            }
        }
        ensure!(body.limit() == 0, "Incomplete archive member");
        let padding = (512 - size % 512) % 512;
        reader.read_exact(&mut [0; 512][..padding as usize])?;
    }
    ensure!(
        pax.is_none() && long_name.is_none(),
        "Extension without an archive member"
    );
    Ok(())
}
