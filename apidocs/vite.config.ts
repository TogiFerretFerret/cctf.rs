import { defineConfig } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";

export default defineConfig({
	plugins: [viteSingleFile()],
	server: {
		proxy: {
			"/openapi.json": "http://127.0.0.1:8080",
			"/openapi.yaml": "http://127.0.0.1:8080",
			"/api": "http://127.0.0.1:8080",
		},
	},
});

