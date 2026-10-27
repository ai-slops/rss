import react from "@vitejs/plugin-react"
import { fileURLToPath } from "node:url"
import { defineConfig } from "vite"

export default defineConfig({
  plugins: [react()],
  resolve: { alias: { "@live-production/gst-channel": fileURLToPath(new URL("./node_modules/gstwebrtc-api/src/com-channel.js", import.meta.url)) } },
  server: {
    proxy: {
      "/api": "http://127.0.0.1:3000",
    },
  },
})
