use anyhow::Result;
use std::mem;
use std::ptr::null_mut;

use winapi::shared::minwindef::{BOOL, DWORD, FALSE};
use winapi::shared::windef::{HDESK__, HGDIOBJ};
use winapi::um::wingdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits,
    SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
};
use winapi::um::winuser::{
    CloseDesktop, GetDC, GetSystemMetrics, OpenInputDesktop, ReleaseDC, SetThreadDesktop,
    SM_CXSCREEN, SM_CYSCREEN,
};

/// Access right needed to switch to a desktop and read its content.
const DESKTOP_SWITCHDESKTOP: DWORD = 0x0100;

/// Screen capture backend using Windows GDI BitBlt.
///
/// Unlike DXGI Desktop Duplication, GDI BitBlt reads directly from the
/// framebuffer of the **thread's assigned desktop**.  Before every capture
/// we call `OpenInputDesktop` + `SetThreadDesktop` so the thread follows
/// whichever desktop currently has user input — including secure desktops
/// created by kiosk apps such as Safe Exam Browser (SEB).
///
/// The API calls are standard, passive read operations — no hooks, no
/// injection, and no window enumeration.
pub struct GdiCapture {
    width: usize,
    height: usize,
    /// Desktop handle currently associated with this thread via
    /// `SetThreadDesktop`.  We hold it so we can close it when the
    /// desktop changes (you cannot `CloseDesktop` the handle that is
    /// currently assigned to your thread).
    held_desktop: *mut HDESK__,
}

impl GdiCapture {
    pub fn new() -> Result<Self> {
        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) } as usize;
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) } as usize;
        if width == 0 || height == 0 {
            return Err(anyhow::anyhow!(
                "GDI: GetSystemMetrics returned zero dimensions"
            ));
        }
        tracing::info!("GDI capture backend initialised: {}×{}", width, height);
        Ok(Self {
            width,
            height,
            held_desktop: null_mut(),
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Captures one frame from the **currently active desktop** via BitBlt
    /// and returns RGB pixel data.
    pub fn capture_frame(&mut self) -> Result<Vec<u8>> {
        unsafe {
            // ── Step 0: Follow the active (input) desktop ──────────────
            // SEB, MSB, and similar kiosk apps create a separate Windows
            // desktop and switch to it.  By following the input desktop,
            // we capture from wherever the user is actually interacting.
            self.follow_input_desktop();

            // ── Step 1: Acquire the desktop device context ─────────────
            let screen_dc = GetDC(null_mut()); // NULL HWND = entire desktop
            if screen_dc.is_null() {
                return Err(anyhow::anyhow!("GDI: GetDC(desktop) returned null"));
            }

            // ── Step 2: Create off-screen DC + bitmap ─────────────────
            let mem_dc = CreateCompatibleDC(screen_dc);
            if mem_dc.is_null() {
                ReleaseDC(null_mut(), screen_dc);
                return Err(anyhow::anyhow!("GDI: CreateCompatibleDC failed"));
            }

            let bmp =
                CreateCompatibleBitmap(screen_dc, self.width as i32, self.height as i32);
            if bmp.is_null() {
                DeleteDC(mem_dc);
                ReleaseDC(null_mut(), screen_dc);
                return Err(anyhow::anyhow!("GDI: CreateCompatibleBitmap failed"));
            }

            let old_obj = SelectObject(mem_dc, bmp as HGDIOBJ);

            // ── Step 3: BitBlt — copy visible screen into our bitmap ──
            let ok = BitBlt(
                mem_dc,
                0,
                0,
                self.width as i32,
                self.height as i32,
                screen_dc,
                0,
                0,
                SRCCOPY,
            );

            if ok == 0 {
                SelectObject(mem_dc, old_obj);
                DeleteObject(bmp as HGDIOBJ);
                DeleteDC(mem_dc);
                ReleaseDC(null_mut(), screen_dc);
                return Err(anyhow::anyhow!("GDI: BitBlt failed"));
            }

            // ── Step 4: Read raw pixel data (BGRA, top-down) ──────────
            let mut bmi: BITMAPINFO = mem::zeroed();
            bmi.bmiHeader.biSize = mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = self.width as i32;
            bmi.bmiHeader.biHeight = -(self.height as i32); // negative → top-down
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB as u32;

            let buf_size = self.width * self.height * 4;
            let mut bgra = vec![0u8; buf_size];

            let lines = GetDIBits(
                mem_dc,
                bmp,
                0,
                self.height as u32,
                bgra.as_mut_ptr() as *mut _,
                &mut bmi,
                DIB_RGB_COLORS as u32,
            );

            // ── Step 5: Release all GDI resources ─────────────────────
            SelectObject(mem_dc, old_obj);
            DeleteObject(bmp as HGDIOBJ);
            DeleteDC(mem_dc);
            ReleaseDC(null_mut(), screen_dc);

            if lines == 0 {
                return Err(anyhow::anyhow!("GDI: GetDIBits returned 0 scan lines"));
            }

            // ── Step 6: Convert BGRA → RGB ────────────────────────────
            let pixel_count = self.width * self.height;
            let mut rgb = Vec::with_capacity(pixel_count * 3);
            for i in 0..pixel_count {
                let off = i * 4;
                rgb.push(bgra[off + 2]); // R
                rgb.push(bgra[off + 1]); // G
                rgb.push(bgra[off]);     // B
            }

            Ok(rgb)
        }
    }

    // ------------------------------------------------------------------
    // Desktop-following logic
    // ------------------------------------------------------------------

    /// Switch this thread to whichever Windows desktop currently has user
    /// input.  This is what makes capture work when SEB (or any kiosk app)
    /// creates a separate secure desktop via `CreateDesktop`/`SwitchDesktop`.
    ///
    /// If `OpenInputDesktop` fails (e.g. the secure desktop restricts
    /// access), we silently stay on the current desktop — no crash.
    unsafe fn follow_input_desktop(&mut self) {
        let new_desk = OpenInputDesktop(
            0,              // dwFlags
            FALSE as BOOL,  // fInherit
            DESKTOP_SWITCHDESKTOP,
        );
        if new_desk.is_null() {
            // Cannot open the input desktop — access denied or other error.
            // Stay on whatever desktop we're currently on.
            return;
        }

        if SetThreadDesktop(new_desk) != 0 {
            // Successfully switched.  Close the previously held handle
            // (we can only CloseDesktop a handle that is NOT the thread's
            // current desktop, so close the OLD one, not the new one).
            if !self.held_desktop.is_null() {
                CloseDesktop(self.held_desktop);
            }
            self.held_desktop = new_desk;
        } else {
            // SetThreadDesktop failed (e.g. thread has hooks on current
            // desktop).  Close the handle we just opened.
            CloseDesktop(new_desk);
        }
    }
}

impl Drop for GdiCapture {
    fn drop(&mut self) {
        unsafe {
            if !self.held_desktop.is_null() {
                // Closing the thread's own desktop handle is harmless if the
                // thread is about to exit; Windows will clean up.
                CloseDesktop(self.held_desktop);
                self.held_desktop = null_mut();
            }
        }
    }
}
