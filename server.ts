import express from "express";
import { exec, spawn } from "child_process";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const app = express();
const PORT = process.env.PORT ? parseInt(process.env.PORT, 10) : 3000;
const isProd = process.env.NODE_ENV === "production";

app.use(express.json({ limit: "5mb" }));

const YOMNOVA_BIN = path.join(__dirname, "yomnova", "target", "debug", "yomnova");

// API: Run code
app.post("/api/run", async (req, res) => {
  const { code } = req.body;
  if (typeof code !== "string") {
    return res.status(400).json({ error: "Missing or invalid 'code' field" });
  }

  const tmpFile = path.join("/tmp", `yomnova_run_${Date.now()}_${Math.random().toString(36).substring(7)}.yom`);
  try {
    await fs.promises.writeFile(tmpFile, code, "utf-8");

    exec(`${YOMNOVA_BIN} run "${tmpFile}" --disassemble`, { timeout: 10000 }, async (err, stdout, stderr) => {
      // Clean up tmp file
      fs.promises.unlink(tmpFile).catch(() => {});

      return res.json({
        success: !err,
        exitCode: err ? (err.code || 1) : 0,
        stdout,
        stderr,
      });
    });
  } catch (error: any) {
    fs.promises.unlink(tmpFile).catch(() => {});
    return res.status(500).json({ error: error.message });
  }
});

// API: Check code (verification only)
app.post("/api/check", async (req, res) => {
  const { code } = req.body;
  if (typeof code !== "string") {
    return res.status(400).json({ error: "Missing or invalid 'code' field" });
  }

  const tmpFile = path.join("/tmp", `yomnova_chk_${Date.now()}_${Math.random().toString(36).substring(7)}.yom`);
  try {
    await fs.promises.writeFile(tmpFile, code, "utf-8");

    exec(`${YOMNOVA_BIN} check "${tmpFile}"`, { timeout: 10000 }, async (err, stdout, stderr) => {
      fs.promises.unlink(tmpFile).catch(() => {});

      return res.json({
        success: !err,
        exitCode: err ? (err.code || 1) : 0,
        stdout,
        stderr,
      });
    });
  } catch (error: any) {
    fs.promises.unlink(tmpFile).catch(() => {});
    return res.status(500).json({ error: error.message });
  }
});

// API: Disassemble code
app.post("/api/disassemble", async (req, res) => {
  const { code } = req.body;
  if (typeof code !== "string") {
    return res.status(400).json({ error: "Missing or invalid 'code' field" });
  }

  const tmpFile = path.join("/tmp", `yomnova_dis_${Date.now()}_${Math.random().toString(36).substring(7)}.yom`);
  try {
    await fs.promises.writeFile(tmpFile, code, "utf-8");

    exec(`${YOMNOVA_BIN} build "${tmpFile}"`, { timeout: 10000 }, async (err, stdout, stderr) => {
      const yomcFile = tmpFile.replace(/\.yom$/, ".yomc");
      let bytecodeListing = "";
      try {
        bytecodeListing = await fs.promises.readFile(yomcFile, "utf-8");
        await fs.promises.unlink(yomcFile);
      } catch (_) {}
      fs.promises.unlink(tmpFile).catch(() => {});

      return res.json({
        success: !err,
        bytecode: bytecodeListing || stdout,
        stderr,
      });
    });
  } catch (error: any) {
    fs.promises.unlink(tmpFile).catch(() => {});
    return res.status(500).json({ error: error.message });
  }
});

// API: Format code
app.post("/api/fmt", async (req, res) => {
  const { code } = req.body;
  if (typeof code !== "string") {
    return res.status(400).json({ error: "Missing or invalid 'code' field" });
  }

  const tmpFile = path.join("/tmp", `yomnova_fmt_${Date.now()}_${Math.random().toString(36).substring(7)}.yom`);
  try {
    await fs.promises.writeFile(tmpFile, code, "utf-8");

    exec(`${YOMNOVA_BIN} fmt "${tmpFile}"`, { timeout: 10000 }, async (err, stdout, stderr) => {
      let formatted = code;
      try {
        formatted = await fs.promises.readFile(tmpFile, "utf-8");
      } catch (_) {}
      fs.promises.unlink(tmpFile).catch(() => {});

      return res.json({
        success: !err,
        formatted,
        stderr,
      });
    });
  } catch (error: any) {
    fs.promises.unlink(tmpFile).catch(() => {});
    return res.status(500).json({ error: error.message });
  }
});

// API: Run workspace cargo tests
app.post("/api/test-suite", (req, res) => {
  exec(`cargo test --manifest-path ${path.join(__dirname, "yomnova", "Cargo.toml")}`, { timeout: 30000 }, (err, stdout, stderr) => {
    return res.json({
      success: !err,
      stdout,
      stderr,
    });
  });
});

// API: Get version and metadata
app.get("/api/version", (req, res) => {
  exec(`${YOMNOVA_BIN} version`, (err, stdout) => {
    res.json({
      version: "0.1.0-dev",
      details: stdout.trim(),
      language: "YOMNOVA",
      tagline: "ONE LANGUAGE. EVERY SYSTEM.",
      creator: "Mouhamed Salim Bousmina",
    });
  });
});

// Examples API
app.get("/api/examples", async (req, res) => {
  const examplesDir = path.join(__dirname, "yomnova", "examples");
  try {
    const files = await fs.promises.readdir(examplesDir);
    const examples = [];
    for (const f of files) {
      if (f.endsWith(".yom")) {
        const content = await fs.promises.readFile(path.join(examplesDir, f), "utf-8");
        examples.push({
          id: f.replace(/\.yom$/, ""),
          filename: f,
          content,
        });
      }
    }
    return res.json(examples);
  } catch (e: any) {
    return res.status(500).json({ error: e.message });
  }
});

// Vite Middleware for dev, or express.static for prod
async function startServer() {
  if (!isProd) {
    const { createServer: createViteServer } = await import("vite");
    const vite = await createViteServer({
      server: { middlewareMode: true },
      appType: "spa",
    });
    app.use(vite.middlewares);
  } else {
    app.use(express.static(path.join(__dirname, "dist")));
    app.get("*", (req, res) => {
      res.sendFile(path.join(__dirname, "dist", "index.html"));
    });
  }

  app.listen(PORT, "0.0.0.0", () => {
    console.log(`YOMNOVA Server listening on port ${PORT} (prod=${isProd})`);
  });
}

startServer();
