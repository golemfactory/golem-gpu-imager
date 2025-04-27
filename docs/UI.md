# UI Specification: Golem GPU Imager

## Purpose
Golem GPU Imager is a desktop utility designed to easily flash official OS images onto Golem GPU devices, or edit existing configurations on already-prepared disks.

---

## Main Modes

- **Flash New Image** (default)
- **Edit Existing Disk**

---

## Application Flow

### 1. Start Screen
- Golem GPU Imager logo.
- Brief description (1–2 sentences).
- Two main options:
    - **"Flash New Image"**
    - **"Edit Existing Disk"**

---

### 2. Flash New Image Flow

#### 2.1 Select OS Image
- Display a list of available official OS images:
    - Thumbnail (optional).
    - OS Name.
    - Version.
    - Short description.
- Features:
    - Search bar (filter by name or version).
    - **"Download"** button or **"Select"** if already downloaded.

#### 2.2 Configure OS Settings
- Optional configuration screen:
    - Hostname.
    - User/root password.
    - Network settings (DHCP / Static IP).
    - SSH options (enable/disable, set public keys).

#### 2.3 Select Target Device
- List of connected storage devices:
    - Device name (e.g., "Kingston 32GB").
    - Path (e.g., `/dev/sdb`).
    - Storage size.
- Warning about data loss.
- **"Write Image"** button.

#### 2.4 Writing Process
- Progress bar showing stages:
    - Image verification.
    - Writing to device.
    - Write verification.
- Live error reporting.
- Cancel option (with confirmation).

#### 2.5 Completion
- Success or failure notification.
- Options:
    - **"Flash Another Device"**
    - **"Exit"**

---

### 3. Edit Existing Disk Flow

#### 3.1 Select Device
- List of connected storage devices.
- After selecting a device:
    - Attempt to recognize Golem GPU OS structure (e.g., `/boot/config.txt`, `golem-config.yaml`).
    - If unrecognized: display warning.

#### 3.2 Read and Display Configuration
- Load available settings:
    - Hostname.
    - Network configuration.
    - SSH settings.
    - Other Golem-specific parameters.

#### 3.3 Edit Configuration
- Editable fields similar to "Configure OS Settings."
- **"Save Changes"** button.
- Automatic backup of previous configuration (`config_backup_DATE.yaml`).

#### 3.4 Completion
- Success or error notification.
- Return to main menu.

---

## UX/UI Requirements

- Single-task focused design.
- No complicated menus or hidden features.
- Maximum 3 clicks from launch to writing or editing.
- Minimalistic icons and clean sans-serif fonts.
- Dark mode as default (light mode optional).
- Support drag & drop for selecting local image files.
- Cross-platform support: Windows, macOS, Linux.
- Use native UI components wherever possible.

---

## Technical Notes

- Must support common filesystems: FAT32, ext4.
- Handle mounting/unmounting safely.
- Ensure versioning and backup when editing existing disks.
- Warn users if changes might require device reboot.

---

## Developer Checklist

- [ ] Implement "Flash New Image" flow.
- [ ] Implement "Edit Existing Disk" flow.
- [ ] Add support for OS image browsing, search, and download.
- [ ] Create configurable settings screen.
- [ ] Detect and list connected storage devices.
- [ ] Handle filesystem mounting and unmounting properly.
- [ ] Implement drag & drop support for local image files.
- [ ] Ensure safe write operations with integrity verification.
- [ ] Implement dark mode as default.
- [ ] Build cross-platform binary (Windows/macOS/Linux).

---

## Designer Checklist

- [ ] Design minimalist and clean UI for each screen.
- [ ] Ensure readability with sans-serif fonts.
- [ ] Optimize layout for 3-click maximum flow.
- [ ] Prepare dark mode and light mode themes.
- [ ] Create visual feedback for each process step (progress, success, errors).
- [ ] Ensure icons are clear, intuitive, and platform-consistent.
