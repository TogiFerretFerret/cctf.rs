interface Doc {
	info?: { title?: string; description?: string; version?: string };
	tags?: { name: string; description?: string }[];
	paths?: Record<string, Record<string, Op>>;
	components?: { schemas?: Record<string, Schema> };
}

interface Op {
	tags?: string[];
	summary?: string;
	description?: string;
	parameters?: Param[];
	requestBody?: { content?: Record<string, { schema?: Schema }> };
	responses?: Record<string, { description?: string; content?: Record<string, { schema?: Schema }> }>;
	security?: unknown[];
}

interface Param { name: string; in: string; required?: boolean; description?: string; schema?: Schema }
type Schema = any;

const METHODS = ["get", "post", "put", "patch", "delete"] as const;
const LANGS = [
	{ code: "en-US", name: "English" },
]; // TODO: Add more languages

const app = document.getElementById("app")!;
let doc: Doc = {};

function h(
	tag: string,
	attrs: Record<string, string> = {},
		...kids: (Node | string | null | undefined)[]
): HTMLElement {
	const el = document.createElement(tag);
	for (const [k, v] of Object.entries(attrs)) {
		if (k === "class") el.className = v;
		else el.setAttribute(k, v);
	}
	for (const kid of kids) if (kid != null) el.append(kid);
	return el;
}

function deref(node: Schema): Schema {
	if (node && typeof node==="object" && node.$ref) {
		const segs = String(node.$ref).replace(/^#\//, "").split("/");
		let cur: any = doc;
		for (const s of segs) cur = cur?.[s];
		return cur??{};
	}
	return node;
}
function typeOf(schema: Schema): string {
	if (!schema) return "any";
	if (schema.$ref) return String(schema.$ref).split("/").pop()!;
	const s=deref(schema);
	if (s.oneOf) return s.oneOf.map(typeOf).join(" | ");
	if (s.type==="array") return `${typeOf(s.items)}[]`;
	if (s.enum) return s.enum.map((e: unknown) => JSON.stringify(e)).join(" | ");
	return s.type??"object";
}

const slug = (s: string) => s.replace(/[^a-z0-9]+/gi, "-").toLowerCase();

function propsTable(schema: Schema): HTMLElement | null {
	const s = deref(schema);
	if (!s?.properties) return null;
	const required: string[] = s.required??[];
	const rows = Object.entries(s.properties).map(([name,p]) => 
		h("tr", {},
		  h("td", { class: "p-name" }, name, required.includes(name) ? h("span", { class: "req" }, "*") : null),
		  h("td", { class: "p-type" }, typeOf(p)),
		  h("td", { class: "p-desc" }, deref(p)?.description ?? ""),
		),
	);
	return h("table", { class: "props" },
			 h("thead", {}, h("tr", {}, h("th", {}, "field"), h("th", {}, "type"), h("th", {}, "description"))),
			 h("tbody", {}, ...rows),
			);
}

function renderOp(path: string, method: string, op: Op): HTMLElement {
	const body: (Node | null)[] = [];
	if (op.description) body.push(h("p", { class: "desc" }, op.description));
	if (op.parameters?.length) {
		body.push(h("h4", {}, "Parameters"));
		body.push(h("table", { class: "props" },
			h("thead", {}, h("tr", {}, h("th", {}, "name"), h("th", {}, "in"), h("th", {}, "type"), h("th", {}, "description"))),
			h("tbody", {}, ...op.parameters.map((p) => 
			   h("tr", {}, 
				 h("td", { class: "p-name" }, p.name, p.required ? h("span", { class: "req" }, "*") : null),
				 h("td", {}, p.in),
				 h("td", { class: "p-type" }, typeOf(p.schema)),
				 h("td", { class: "p-desc" }, p.description ?? ""),
				))),
			));
	}
	const reqSchema = op.requestBody?.content?.["application/json"]?.schema;
	if (reqSchema) {
		body.push(h("h4", {}, "Request body"), h("code", { class: "typeline" }, typeOf(reqSchema)), propsTable(reqSchema));
	}
	if (op.responses) {
		body.push(h("h4", {}, "Responses"));
		for (const [code, resp] of Object.entries(op.responses)) {
			const schema = resp.content?.["application/json"]?.schema;
			body.push(h("div", { class: "resp" },
				h("span", { class: `code code-${code[0]}` }, code),
				h("span", { class: "resp-desc" }, resp.description ?? ""),
				schema ? h("code", { class: "typeline" }, typeOf(schema)) : null, 
			));
		}
	}
	const authed = !(Array.isArray(op.security) && op.security.length === 0);
	return h("section", { class: "op", id: `op-${method}-${slug(path)}` },
		h("div", { class: "op-head" },
			h("span", { class: `badge ${method}` }, method.toUpperCase()),
			h("code", { class: "path" }, path),
			authed ? h("span", { class: "lock", title: "Requires auth" }, "🔒") : null,
			h("span", { class: "op-summary" }, op.summary ?? ""),
		),
		h("div", { class: "op-body" }, ...body.filter((n): n is Node => n != null)),
	);
}

function opsByTag(): Map<string, { path: string; method: string; op: Op }[]> {
	const map = new Map<string, { path: string; method: string; op: Op }[]>();
	for (const [path, item] of Object.entries(doc.paths??{})) {
		for (const method of METHODS) {
			const op = item[method];
			if (!op) continue;
			const tag = op.tags?.[0] ?? "default";
			if (!map.has(tag)) map.set(tag, []);
			map.get(tag)!.push({ path, method, op });
		}
	}
	return map;
}

function langSwitcher(): HTMLSelectElement {
	const sel = h("select", { class: "lang", "aria-label": "Language" }) as HTMLSelectElement;
	const current = new URLSearchParams(location.search).get("lang") ?? "en-US";
	for (const l of LANGS) {
		const o = h("option", { value: l.code }, l.name) as HTMLOptionElement;
		if (l.code===current) o.selected = true;
		sel.append(o);
	}
	sel.addEventListener("change", () => {
		const u = new URL(location.href);
		u.searchParams.set("lang", sel.value);
		location.href = u.toString();
	});
	return sel;
}

function render(): void {
	const byTag = opsByTag();
	const nav = h("nav", { class: "nav" });
	for (const [tag, ops] of byTag) {
		nav.append(h("div", { class: "nav-tag" }, tag));
		for (const { path, method } of ops) {
			nav.append(h("a", { class: "nav-op", href: `#op-${method}-${slug(path)}` },
				h("span", { class: `badge sm ${method}` }, method.toUpperCase()),
				h("span", { class: "nav-path" }, path),
			));
		}
	}
	const sidebar = h("aside", { class: "sidebar" },
		h("div", { class: "brand" }, doc.info?.title ?? "API",
		  doc.info?.version ? h("span", { class: "ver" }, `v${doc.info.version}`) : null),
		langSwitcher(),
		nav,
	);
	const main = h("main", { class: "main" },
		h("header", { class: "info" },
		  h("h1", {}, doc.info?.title ?? "API"),
		  doc.info?.description ? h("p", { class: "desc" }, doc.info.description) : null,
		),
	);
	for (const [tag, ops] of byTag) {
		const meta = doc.tags?.find((t) => t.name === tag);
		main.append(h("section", { class: "tag-section" },
			h("h2", { id: `tag-${slug(tag)}` }, tag),
			meta?.description ? h("p", { class: "desc" }, meta.description) : null,
			...ops.map(({ path, method, op }) => renderOp(path, method, op)),
		));
	}
	const schemas = doc.components?.schemas ?? {};
	if (Object.keys(schemas).length) {
		const sec = h("section", { class: "tag-section" }, h("h2", { id: "schemas" }, "Schemas"));
		for (const [name, schema] of Object.entries(schemas)) {
			sec.append(h("section", { class: "op", id: `schema-${name}` },
				 h("div", { class: "op-head" }, h("code", { class: "path" }, name),
				   h("span", { class: "op-summary" }, typeOf(schema))),
				 h("div", { class: "op-body" },
				   deref(schema)?.description ? h("p", { class: "desc" }, deref(schema).description) : null, 
				   propsTable(schema),
				),
			));
		}
		main.append(sec);
	}
	app.replaceChildren(h("div", { class: "layout" }, sidebar, main));
}

async function boot(): Promise<void> {
	const lang = new URLSearchParams(location.search).get("lang") ?? "en-US";
	try {
		const res = await fetch(`/openapi.json?lang=${encodeURIComponent(lang)}`);
		if (!res.ok) throw new Error(`HTTP ${res.status}`);
		doc = await res.json();
		render();
	} catch (e) {
		app.replaceChildren(h("div", { class: "error" },
			`Failed to load /openapi.json - is cctf.rs running on :8080? (${e})`));
	}
}
boot();
