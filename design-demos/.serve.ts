const root = "D:/codes/GiantappMan/eye-nurse/design-demos";
Bun.serve({
  port: 8123,
  async fetch(req) {
    let p = decodeURIComponent(new URL(req.url).pathname);
    if (p === "/") p = "/00-目录.html";
    const f = Bun.file(root + p);
    if (await f.exists()) return new Response(f);
    return new Response("not found", { status: 404 });
  },
});
console.log("serving on 8123");
