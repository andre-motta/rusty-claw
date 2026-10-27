//! FFmpeg reading through any Rust reader (a file, an archive entry, bytes
//! in memory), so no file is ever opened by FFmpeg itself.

use std::ffi::{c_int, c_void};
use std::io::{self, SeekFrom};
use std::panic::{self, AssertUnwindSafe};

use rusty_ffmpeg::ffi;

use crate::{MovieError, ReadSeek};

/// FFmpeg's read buffer size.
const BUFFER: usize = 64 * 1024;

type Reader = Box<dyn ReadSeek + Send>;

/// An FFmpeg I/O context over a reader. Dropping it frees both.
pub(super) struct CustomIo {
    pub(super) ctx: *mut ffi::AVIOContext,
    reader: *mut Reader,
}

impl CustomIo {
    pub(super) fn new(reader: Reader) -> Result<CustomIo, MovieError> {
        let reader = Box::into_raw(Box::new(reader));
        // SAFETY: av_malloc returns memory FFmpeg can own or NULL.
        let buffer = unsafe { ffi::av_malloc(BUFFER) } as *mut u8;
        if buffer.is_null() {
            // SAFETY: `reader` came from Box::into_raw above.
            drop(unsafe { Box::from_raw(reader) });
            return Err(MovieError::Open("out of memory".into()));
        }
        // SAFETY: the buffer is FFmpeg memory of BUFFER bytes; the opaque
        // pointer stays valid until Drop, which frees the context first.
        let ctx = unsafe {
            ffi::avio_alloc_context(
                buffer,
                BUFFER as c_int,
                0,
                reader as *mut c_void,
                Some(read_packet),
                None,
                Some(seek),
            )
        };
        if ctx.is_null() {
            // SAFETY: both were allocated above and handed to nobody.
            unsafe {
                ffi::av_free(buffer as *mut c_void);
                drop(Box::from_raw(reader));
            }
            return Err(MovieError::Open("out of memory".into()));
        }
        Ok(CustomIo { ctx, reader })
    }
}

impl Drop for CustomIo {
    fn drop(&mut self) {
        // SAFETY: the context and the reader were made in `new` and are
        // freed once here. FFmpeg may have replaced the buffer, so the one
        // the context holds now is freed.
        unsafe {
            if !self.ctx.is_null() {
                ffi::av_freep(&mut (*self.ctx).buffer as *mut *mut u8 as *mut c_void);
                ffi::avio_context_free(&mut self.ctx);
            }
            drop(Box::from_raw(self.reader));
        }
    }
}

fn reader<'a>(opaque: *mut c_void) -> &'a mut Reader {
    // SAFETY: FFmpeg passes back the opaque pointer given in `new`, which
    // points to a live reader that only the decoding thread touches.
    unsafe { &mut *(opaque as *mut Reader) }
}

unsafe extern "C" fn read_packet(opaque: *mut c_void, buf: *mut u8, size: c_int) -> c_int {
    if buf.is_null() || size <= 0 {
        return 0;
    }
    // SAFETY: FFmpeg hands a writable buffer of `size` bytes.
    let out = unsafe { std::slice::from_raw_parts_mut(buf, size as usize) };
    let r = reader(opaque);
    let got = panic::catch_unwind(AssertUnwindSafe(|| {
        loop {
            match r.read(out) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                other => return other,
            }
        }
    }));
    match got {
        Ok(Ok(0)) => ffi::AVERROR_EOF,
        Ok(Ok(n)) => n as c_int,
        _ => ffi::AVERROR(ffi::EIO),
    }
}

unsafe extern "C" fn seek(opaque: *mut c_void, offset: i64, whence: c_int) -> i64 {
    let r = reader(opaque);
    let whence = whence & !(ffi::AVSEEK_FORCE as c_int);
    let got = panic::catch_unwind(AssertUnwindSafe(|| -> io::Result<u64> {
        if whence == ffi::AVSEEK_SIZE as c_int {
            let here = r.stream_position()?;
            let end = r.seek(SeekFrom::End(0))?;
            r.seek(SeekFrom::Start(here))?;
            return Ok(end);
        }
        let to = match whence as u32 {
            ffi::SEEK_SET => SeekFrom::Start(
                u64::try_from(offset).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?,
            ),
            ffi::SEEK_CUR => SeekFrom::Current(offset),
            ffi::SEEK_END => SeekFrom::End(offset),
            _ => return Err(io::Error::from(io::ErrorKind::InvalidInput)),
        };
        r.seek(to)
    }));
    match got {
        Ok(Ok(at)) => i64::try_from(at).unwrap_or(i64::MAX),
        _ => i64::from(ffi::AVERROR(ffi::EIO)),
    }
}
