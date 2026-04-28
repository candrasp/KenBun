// Contoh script Bun yang akan dijalankan oleh Ken App
// Ganti isi file ini dengan logika Anda sendiri

const PORT = process.env.PORT || 3000;

const server = Bun.serve({
  port: PORT,
  fetch(req) {
    const url = new URL(req.url);
    console.log(`[${new Date().toISOString()}] ${req.method} ${url.pathname}`);
    return new Response("Hello from KenBun!", { status: 200 });
  },
});

console.log(`✅ Bun server berjalan di http://localhost:${PORT}`);

// Jaga proses tetap hidup
process.on("SIGINT", () => {
  console.log("⛔ Bun server dihentikan.");
  server.stop();
  process.exit(0);
});
