# KenBun 🚀
**Manajer Server Bun Portabel & Ringan untuk Windows**

KenBun adalah alat bantu (utility) untuk menjalankan script **Bun** di latar belakang tanpa harus membuka terminal. Sangat cocok bagi pengembang yang ingin kemudahan seperti Laragon namun khusus untuk ekosistem Bun.

---

<p align="center">
  <img src="screenshot.png" alt="KenBun Screenshot" width="500">
</p>

## ✨ Fitur Unggulan
*   **📍 Portabel:** Pindahkan file `.exe` ke folder project mana saja, ia akan otomatis mengenali project tersebut.
*   **📥 Minimalkan ke Tray:** Menutup jendela tidak mematikan server. Aplikasi akan tetap aktif di pojok bawah layar (System Tray).
*   **📟 Terminal Internal:** Lihat log/aktivitas server Anda langsung di dalam aplikasi tanpa terminal hitam.
*   **🛡️ Anti-Bentrokan Port:** Otomatis membersihkan port yang macet sebelum server baru dijalankan.
*   **⚙️ Pengaturan Fleksibel:** Simpan nomor port favorit Anda untuk setiap project.

---

## 🛠️ Langkah-Langkah Persiapan (Penting!)

Sebelum menggunakan KenBun, pastikan dua hal ini sudah siap:

### 1. Instal Bun
Pastikan komputer Anda sudah memiliki **Bun**. Jika belum, instal via PowerShell:
```powershell
powershell -c "irm bun.sh/install.ps1 | iex"
```

### 2. Siapkan file `index.js`
KenBun akan mencari file bernama `index.js` di foldernya. Agar fitur **Custom Port** berfungsi, kode Anda harus menggunakan format ini:

```javascript
// WAJIB: Membaca Port dari sistem
const PORT = process.env.PORT || 3000;

const server = Bun.serve({
  port: PORT, // Gunakan variabel PORT di sini
  fetch(req) {
    return new Response("Halo dari KenBun!");
  },
});

console.log(`Server aktif di: http://localhost:${PORT}`);
```

---

## 🚀 Cara Menggunakan KenBun

1.  **Taruh & Buka:** Masukkan file `KenBun.exe` ke folder project Anda, lalu klik dua kali untuk membukanya.
2.  **Atur Port (Opsional):** Jika ingin menggunakan port selain 3000, klik ikon **Gear (⚙️)** di pojok kanan atas, masukkan angka port, dan klik **Simpan**.
3.  **Klik Start:** Klik tombol **Start** di halaman utama. 
    *   *KenBun akan otomatis membuka browser untuk Anda.*
4.  **Pantau Aktivitas:** Klik ikon **Terminal (>_)** untuk melihat log server Anda secara real-time.
5.  **Sembunyikan:** Klik tombol **X** pada jendela aplikasi jika ingin menyembunyikannya ke System Tray (dekat jam/hidden icon).
6.  **Matikan:** Klik tombol **Stop** di aplikasi, atau klik kanan ikon KenBun di System Tray lalu pilih **Keluar**.

---

## 🧩 Teknologi
Dibangun dengan **Tauri v2** menggunakan bahasa pemrograman **Rust** dan **JavaScript** untuk memastikan performa yang sangat cepat dan penggunaan RAM yang minimal (~40MB).

---
Dibuat dengan ❤️ untuk memudahkan pengembang Bun di Windows.
