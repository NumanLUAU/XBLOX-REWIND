/*
	written by: @corescripts
*/

// >> constants
const REPO_API = "https://api.github.com/repos/NumanLUAU/XBLOX-REWIND/releases/latest";
const ZONES = ["City", "Space", "Volcano"];
const ZONE_LENGTH = 60;			// seconds per zone, like CameraManager
const ZONE_FADE = 0.85;			// half of the 1.7 s transition between zones
const CAMERA_RATE = 4;			// world.js holds a camera every 0.25 s
const MUSIC_VOLUME = 0.55;
const MUSIC_FADE = 3;			// seconds to fade the music in
const RENDER_SCALE = 0.32;		// the menu world is blurred anyway: draw it small, let the browser scale it up
const SUN_STRENGTH = 1.9;		// stronger than the launcher's sun so the light still reads through the filter
const AMBIENT_STRENGTH = 1.15;
const FILTER_BRIGHTNESS = 0.4;	// the launcher's flythrough brightness is 0.3
const GLOW = 0.55;				// bloom strength for bright light
const MAX_RENDER_WIDTH = 900;
const RAW = new URLSearchParams(location.search).has("raw");	// ?raw: no filter or blur (checking the world)
const SEEK = Number(new URLSearchParams(location.search).get("t") || 0);	// ?t=seconds into the flythrough
const AUTO_BEGIN = new URLSearchParams(location.search).has("begin");	// ?begin: skip "Press A" (screenshots)

// >> variables
let music = null;
let musicMuted = false;
let musicFade = null;
let begun = false;
let currentTab = "home";
let selected = null;
let lastMoveSound = 0;

// >> audio
// plain <audio> elements, so sound also works with the page opened straight from disk (file://),
// where Web Audio cannot read local files

const SOUND_FILES = {
	move: "assets/sounds/MoveSelection.mp3",
	press: "assets/sounds/ButtonPress.mp3",
	screen: "assets/sounds/ScreenChange.mp3",
};
const sounds = {};
for (const [name, src] of Object.entries(SOUND_FILES)) {
	sounds[name] = new Audio(src);
	sounds[name].preload = "auto";
}

function playSound(name, volume = 0.6) {
	const base = sounds[name];
	if (!begun || !base) return;
	// a copy per play, so quick presses overlap instead of cutting each other off
	const s = base.cloneNode();
	s.volume = volume;
	s.play().catch(() => {});
}

// ease the music's volume to `target` over `seconds`
function fadeMusic(target, seconds) {
	if (!music) return;
	cancelAnimationFrame(musicFade);
	const from = music.volume;
	const t0 = performance.now();
	// time is read here, not from the frame's timestamp (which can be a moment before t0); a
	// negative step would set a negative volume, which throws and stops the fade at silence
	const step = () => {
		const k = Math.max(0, Math.min(1, (performance.now() - t0) / (seconds * 1000)));
		music.volume = Math.max(0, Math.min(1, from + (target - from) * k));
		if (k < 1) musicFade = requestAnimationFrame(step);
	};
	musicFade = requestAnimationFrame(step);
}

// the theme loops and fades in from silence, like the console's menu music
function startMusic() {
	music = new Audio("assets/sounds/RobloxMusic.ogg");
	music.loop = true;
	music.volume = 0;
	music.play().then(() => fadeMusic(musicMuted ? 0 : MUSIC_VOLUME, MUSIC_FADE)).catch(() => {});
}

function setMuted(muted) {
	musicMuted = muted;
	document.getElementById("mute").classList.toggle("off", muted);
	fadeMusic(muted ? 0 : MUSIC_VOLUME, 0.6);
}

// >> engagement
// the first click, key or controller button anywhere starts everything

function begin() {
	if (begun) return;
	begun = true;
	startMusic();
	playSound("screen");
	const engage = document.getElementById("engage");
	engage.classList.add("leaving");
	setTimeout(() => {
		engage.hidden = true;
		document.getElementById("shell").hidden = false;
		showTab("home", true);
	}, 450);
}

// >> shell navigation

function navItems() {
	const pane = document.querySelector(`.pane[data-pane="${currentTab}"]`);
	return [...document.querySelectorAll(".top [data-nav]"), ...pane.querySelectorAll("[data-nav]")];
}

function select(el, withSound = true) {
	if (el === selected) return;
	if (selected) selected.classList.remove("selected");
	selected = el;
	if (!el) return;
	el.classList.add("selected");
	el.scrollIntoView({ block: "nearest", behavior: "smooth" });
	const now = performance.now();
	if (withSound && now - lastMoveSound > 60) {
		lastMoveSound = now;
		playSound("move", 0.45);
	}
}

function showTab(name, quiet = false) {
	if (!quiet && name === currentTab) return;
	currentTab = name;
	for (const pane of document.querySelectorAll(".pane")) {
		const on = pane.dataset.pane === name;
		pane.hidden = !on;
		// restart the slide-in each time the pane opens
		if (on) {
			pane.style.animation = "none";
			void pane.offsetWidth;
			pane.style.animation = "";
		}
	}
	for (const tab of document.querySelectorAll(".tab")) tab.classList.toggle("current", tab.dataset.tab === name);
	if (!quiet) playSound("screen");
	select(document.querySelector(`.tab[data-tab="${name}"]`), false);
}

function activate(el) {
	if (!el) return;
	playSound("press");
	if (el.dataset.tab) return showTab(el.dataset.tab);
	if (el.id === "mute") return setMuted(!musicMuted);
	if (el.dataset.full) return openViewer(el.dataset.full);
	if (el.tagName === "A") window.open(el.href, "_blank", "noopener");
}

// arrow keys / d-pad: the nearest item in that direction
function move(dx, dy) {
	const items = navItems();
	if (!selected || !items.includes(selected)) return select(items[0]);
	const from = selected.getBoundingClientRect();
	const fx = from.left + from.width / 2;
	const fy = from.top + from.height / 2;
	let best = null;
	let bestScore = Infinity;
	for (const el of items) {
		if (el === selected) continue;
		const r = el.getBoundingClientRect();
		const ex = r.left + r.width / 2 - fx;
		const ey = r.top + r.height / 2 - fy;
		const along = ex * dx + ey * dy;
		if (along <= 4) continue;
		const across = Math.abs(ex * dy - ey * dx);
		const score = along + across * 2;
		if (score < bestScore) {
			bestScore = score;
			best = el;
		}
	}
	if (best) select(best);
}

function back() {
	const viewer = document.getElementById("viewer");
	if (!viewer.hidden) return closeViewer();
	playSound("press");
	select(document.querySelector(`.tab[data-tab="${currentTab}"]`), false);
}

function cycleTab(step) {
	const tabs = [...document.querySelectorAll(".tab")].map((t) => t.dataset.tab);
	showTab(tabs[(tabs.indexOf(currentTab) + step + tabs.length) % tabs.length]);
}

function openViewer(src) {
	const viewer = document.getElementById("viewer");
	viewer.querySelector("img").src = src;
	viewer.hidden = false;
}

function closeViewer() {
	document.getElementById("viewer").hidden = true;
}

// >> input

function onKey(e) {
	if (!begun) {
		if (!e.ctrlKey && !e.metaKey && !e.altKey) begin();
		return;
	}
	const keys = {
		ArrowLeft: () => move(-1, 0), ArrowRight: () => move(1, 0), ArrowUp: () => move(0, -1), ArrowDown: () => move(0, 1),
		a: () => move(-1, 0), d: () => move(1, 0), w: () => move(0, -1), s: () => move(0, 1),
		Enter: () => activate(selected), " ": () => activate(selected),
		Escape: back, Backspace: back,
		q: () => cycleTab(-1), e: () => cycleTab(1),
	};
	const fn = keys[e.key];
	if (fn) {
		e.preventDefault();
		fn();
	}
}

// controllers: d-pad / stick, A, B, LB, RB (polled once a frame)
const padState = {};
function pollPads() {
	const pads = navigator.getGamepads ? navigator.getGamepads() : [];
	for (const pad of pads) {
		if (!pad) continue;
		const prev = padState[pad.index] || {};
		const now = {
			up: pad.buttons[12]?.pressed || pad.axes[1] < -0.6,
			down: pad.buttons[13]?.pressed || pad.axes[1] > 0.6,
			left: pad.buttons[14]?.pressed || pad.axes[0] < -0.6,
			right: pad.buttons[15]?.pressed || pad.axes[0] > 0.6,
			a: pad.buttons[0]?.pressed, b: pad.buttons[1]?.pressed,
			lb: pad.buttons[4]?.pressed, rb: pad.buttons[5]?.pressed,
		};
		const hit = (k) => now[k] && !prev[k];
		if (!begun) {
			if (Object.keys(now).some(hit)) begin();
		} else {
			if (hit("up")) move(0, -1);
			if (hit("down")) move(0, 1);
			if (hit("left")) move(-1, 0);
			if (hit("right")) move(1, 0);
			if (hit("a")) activate(selected);
			if (hit("b")) back();
			if (hit("lb")) cycleTab(-1);
			if (hit("rb")) cycleTab(1);
		}
		padState[pad.index] = now;
	}
}

// >> release info

function loadRelease() {
	fetch(REPO_API)
		.then((r) => (r.ok ? r.json() : Promise.reject()))
		.then((rel) => {
			const date = new Date(rel.published_at).toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" });
			const asset = (rel.assets || []).find((a) => a.name.endsWith(".zip"));
			for (const el of document.querySelectorAll("[data-release-line]")) el.textContent = `${rel.name || rel.tag_name} · ${date}`;
			if (asset) for (const el of document.querySelectorAll("[data-release-link]")) el.href = asset.browser_download_url;
		})
		.catch(() => {});
}

// >> math (column-major 4x4)

function perspective(fovY, aspect, near, far) {
	const f = 1 / Math.tan(fovY / 2);
	const nf = 1 / (near - far);
	return new Float32Array([f / aspect, 0, 0, 0, 0, f, 0, 0, 0, 0, (far + near) * nf, -1, 0, 0, 2 * far * near * nf, 0]);
}

function lookAt(eye, dir) {
	const norm = (v) => {
		const l = Math.hypot(v[0], v[1], v[2]) || 1;
		return [v[0] / l, v[1] / l, v[2] / l];
	};
	const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
	const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
	const z = norm([-dir[0], -dir[1], -dir[2]]);
	const x = norm(cross([0, 1, 0], z));
	const y = cross(z, x);
	return new Float32Array([x[0], y[0], z[0], 0, x[1], y[1], z[1], 0, x[2], y[2], z[2], 0, -dot(x, eye), -dot(y, eye), -dot(z, eye), 1]);
}

function multiply(a, b) {
	const out = new Float32Array(16);
	for (let c = 0; c < 4; c++) {
		for (let r = 0; r < 4; r++) {
			out[c * 4 + r] = a[r] * b[c * 4] + a[4 + r] * b[c * 4 + 1] + a[8 + r] * b[c * 4 + 2] + a[12 + r] * b[c * 4 + 3];
		}
	}
	return out;
}

// >> world renderer
// the launcher's menu world (Map.rbxl): every part and the smooth terrain, baked into one mesh by
// XbloxRewind.exe --export-web, drawn in one call with the launcher's lighting and menu filter

const VERTEX_SHADER = `#version 300 es
in vec3 aPos;
in vec4 aNormal;
in vec4 aColor;
uniform mat4 uViewProj;
uniform vec3 uLo;
uniform vec3 uExt;
out vec3 vWorld;
out vec3 vNormal;
out vec3 vColor;
void main() {
	vWorld = uLo + aPos * uExt;
	vNormal = aNormal.xyz;
	vColor = aColor.rgb;
	gl_Position = uViewProj * vec4(vWorld, 1.0);
}`;

// lighting as in the launcher's scene.wgsl (2016 "compatibility" look), then its menu filter
const FRAGMENT_SHADER = `#version 300 es
precision highp float;
in vec3 vWorld;
in vec3 vNormal;
in vec3 vColor;
uniform vec3 uCamera;
uniform vec3 uSun;
uniform vec3 uAmbient;
uniform vec3 uOutdoor;
uniform vec3 uShiftTop;
uniform vec3 uShiftBottom;
uniform float uBrightness;
uniform vec2 uStrength;
uniform float uGlow;
uniform vec3 uFogColor;
uniform vec2 uFog;
uniform vec4 uFilter;
uniform vec3 uTint;
out vec4 outColor;
float hash(vec3 p) { return fract(sin(dot(p, vec3(12.9898, 78.233, 37.719))) * 43758.5453); }
float noise(vec3 p) {
	vec3 i = floor(p);
	vec3 f = fract(p);
	f = f * f * (3.0 - 2.0 * f);
	return mix(mix(mix(hash(i), hash(i + vec3(1, 0, 0)), f.x), mix(hash(i + vec3(0, 1, 0)), hash(i + vec3(1, 1, 0)), f.x), f.y),
		mix(mix(hash(i + vec3(0, 0, 1)), hash(i + vec3(1, 0, 1)), f.x), mix(hash(i + vec3(0, 1, 1)), hash(i + vec3(1, 1, 1)), f.x), f.y), f.z);
}
void main() {
	vec3 n = normalize(gl_FrontFacing ? vNormal : -vNormal);
	vec3 base = vColor * (0.82 + 0.3 * noise(vWorld * 0.12));
	float ndl = dot(n, uSun);
	float skyVis = 0.35 + 0.3 * clamp(n.y * 0.5 + 0.5, 0.0, 1.0);
	vec3 sunColor = (vec3(0.8) * uBrightness + uShiftTop * 0.7) * uStrength.x;
	vec3 light = (uAmbient + uOutdoor * skyVis) * uStrength.y + sunColor * max(ndl, 0.0) + uShiftBottom * max(-ndl, 0.0);
	// a soft rim where surfaces turn away from the camera, like light wrapping round the edges
	vec3 view = normalize(uCamera - vWorld);
	light += sunColor * 0.25 * pow(1.0 - max(dot(n, view), 0.0), 3.0) * max(dot(-view, uSun) * 0.5 + 0.5, 0.0);
	vec3 c = base * light;
	float fog = clamp((length(uCamera - vWorld) - uFog.x) / max(uFog.y - uFog.x, 1.0), 0.0, 1.0);
	c = mix(c, uFogColor, fog);
	// ColorCorrectionEffect: saturation, contrast, brightness, tint (uFilter = saturation, contrast, brightness, fade)
	float lum = dot(c, vec3(0.299, 0.587, 0.114));
	c = mix(vec3(lum), c, 1.0 + uFilter.x);
	c = (c - 0.5) * (1.0 + uFilter.y) + 0.5 + uFilter.z;
	c = clamp(c * uTint, 0.0, 1.0);
	// bloom: bright light (sunlit faces, neon, water) glows through the tint instead of being capped by it
	c += vec3(0.32, 0.62, 0.95) * pow(clamp(lum, 0.0, 1.0), 2.2) * uGlow;
	c *= uFilter.w;
	outColor = vec4(c, 1.0);
}`;

function compile(gl, type, source) {
	const s = gl.createShader(type);
	gl.shaderSource(s, source);
	gl.compileShader(s);
	if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
	return s;
}

// assets/world/world.js (a plain <script>, so this works from disk too) sets window.XBLOX_WORLD:
// the zones and the mesh, gzipped and base64 encoded
async function loadWorld() {
	const world = window.XBLOX_WORLD;
	if (!world) throw new Error("assets/world/world.js did not load");
	const packed = Uint8Array.from(atob(world.mesh), (c) => c.charCodeAt(0));
	const stream = new Blob([packed]).stream().pipeThrough(new DecompressionStream("gzip"));
	const buffer = await new Response(stream).arrayBuffer();
	delete world.mesh;
	return { zones: world.zones, buffer };
}

async function startWorld() {
	const canvas = document.getElementById("world");
	const gl = canvas.getContext("webgl2", { antialias: false, alpha: false, depth: true, powerPreference: "low-power" });
	if (!gl || typeof DecompressionStream === "undefined") return false;
	const { zones, buffer } = await loadWorld();
	const head = new DataView(buffer);
	if (String.fromCharCode(...new Uint8Array(buffer, 0, 4)) !== "XBW1") return false;
	const vertexCount = head.getUint32(4, true);
	const indexCount = head.getUint32(8, true);
	const lo = [head.getFloat32(12, true), head.getFloat32(16, true), head.getFloat32(20, true)];
	const ext = [head.getFloat32(24, true), head.getFloat32(28, true), head.getFloat32(32, true)];
	const posAt = 36;
	const nrmAt = posAt + vertexCount * 6;
	const colAt = nrmAt + vertexCount * 4;
	const idxAt = colAt + vertexCount * 4;

	const program = gl.createProgram();
	gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, VERTEX_SHADER));
	gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER));
	gl.linkProgram(program);
	if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(program));
	gl.useProgram(program);
	const u = {};
	for (const name of ["uViewProj", "uLo", "uExt", "uCamera", "uSun", "uAmbient", "uOutdoor", "uShiftTop", "uShiftBottom", "uBrightness", "uStrength", "uGlow", "uFogColor", "uFog", "uFilter", "uTint"]) {
		u[name] = gl.getUniformLocation(program, name);
	}

	// one vertex array: positions (u16 x3), normals (i8 x4), colours (u8 x4), u32 indices
	const vao = gl.createVertexArray();
	gl.bindVertexArray(vao);
	const attribute = (name, size, type, at, bytes) => {
		const loc = gl.getAttribLocation(program, name);
		const buf = gl.createBuffer();
		gl.bindBuffer(gl.ARRAY_BUFFER, buf);
		gl.bufferData(gl.ARRAY_BUFFER, new Uint8Array(buffer, at, bytes), gl.STATIC_DRAW);
		gl.enableVertexAttribArray(loc);
		gl.vertexAttribPointer(loc, size, type, true, 0, 0);
	};
	attribute("aPos", 3, gl.UNSIGNED_SHORT, posAt, vertexCount * 6);
	attribute("aNormal", 4, gl.BYTE, nrmAt, vertexCount * 4);
	attribute("aColor", 4, gl.UNSIGNED_BYTE, colAt, vertexCount * 4);
	const ibo = gl.createBuffer();
	gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ibo);
	gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, new Uint8Array(buffer, idxAt, indexCount * 4), gl.STATIC_DRAW);
	gl.uniform3fv(u.uLo, lo);
	gl.uniform3fv(u.uExt, ext);
	gl.enable(gl.DEPTH_TEST);

	// size: a fraction of the window, smaller again if frames run slow
	let scale = RENDER_SCALE;
	const resize = () => {
		const w = Math.min(Math.round(innerWidth * scale), MAX_RENDER_WIDTH);
		const h = Math.round(w * innerHeight / innerWidth);
		if (canvas.width !== w || canvas.height !== h) {
			canvas.width = w;
			canvas.height = h;
		}
	};
	addEventListener("resize", resize);
	resize();

	const start = performance.now();
	let slowFrames = 0;
	let last = start;
	const frame = (now) => {
		requestAnimationFrame(frame);
		pollPads();
		if (document.hidden) return;
		const dt = now - last;
		last = now;
		if (dt > 26 && ++slowFrames > 90 && scale > 0.25) {
			scale = 0.25;
			resize();
		}
		// (the first frame's timestamp can be a moment before `start`)
		const t = Math.max(0, (now - start) / 1000) + SEEK;
		const zone = zones[ZONES[Math.floor(t / ZONE_LENGTH) % ZONES.length]];
		const local = t % ZONE_LENGTH;
		const cams = zone.cameras;
		const f = Math.min(local * CAMERA_RATE, cams.length - 1.001);
		const i = Math.floor(f);
		const k = f - i;
		const a = cams[i];
		const b = cams[i + 1];
		const eye = [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k];
		const dir = [a[3] + (b[3] - a[3]) * k, a[4] + (b[4] - a[4]) * k, a[5] + (b[5] - a[5]) * k];
		// fade through black between zones, as the menu does
		const fade = Math.max(0, Math.min(1, local / ZONE_FADE, (ZONE_LENGTH - local) / ZONE_FADE));
		const filter = zone.filter;
		gl.viewport(0, 0, canvas.width, canvas.height);
		const fog = zone.fogColor;
		const lum = fog[0] * 0.299 + fog[1] * 0.587 + fog[2] * 0.114;
		const sky = (lum - 0.5) * (1 + filter.contrast) + 0.5 + FILTER_BRIGHTNESS;
		gl.clearColor(sky * filter.tint[0] * fade, sky * filter.tint[1] * fade, sky * filter.tint[2] * fade, 1);
		gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
		gl.uniformMatrix4fv(u.uViewProj, false, multiply(perspective(70 * Math.PI / 180, canvas.width / canvas.height, 0.5, 4000), lookAt(eye, dir)));
		gl.uniform3fv(u.uCamera, eye);
		gl.uniform3fv(u.uSun, zone.sun);
		gl.uniform3fv(u.uAmbient, zone.ambient);
		gl.uniform3fv(u.uOutdoor, zone.outdoorAmbient);
		gl.uniform3fv(u.uShiftTop, zone.shiftTop);
		gl.uniform3fv(u.uShiftBottom, zone.shiftBottom);
		gl.uniform1f(u.uBrightness, zone.brightness);
		gl.uniform2f(u.uStrength, SUN_STRENGTH, AMBIENT_STRENGTH);
		gl.uniform1f(u.uGlow, RAW ? 0 : GLOW);
		gl.uniform3fv(u.uFogColor, fog);
		gl.uniform2f(u.uFog, zone.fogStart, zone.fogEnd);
		if (RAW) {
			gl.uniform4f(u.uFilter, 0, 0, 0, 1);
			gl.uniform3fv(u.uTint, [1, 1, 1]);
		} else {
			gl.uniform4f(u.uFilter, filter.saturation, filter.contrast, FILTER_BRIGHTNESS, fade);
			gl.uniform3fv(u.uTint, filter.tint);
		}
		gl.drawElements(gl.TRIANGLES, indexCount, gl.UNSIGNED_INT, 0);
	};
	requestAnimationFrame(frame);
	canvas.classList.add("ready");
	if (RAW) canvas.style.filter = "none";
	return true;
}

// >> main

document.addEventListener("pointerdown", (e) => {
	if (!begun) {
		e.preventDefault();
		begin();
	}
}, true);
document.addEventListener("keydown", onKey);
document.addEventListener("click", (e) => {
	if (!begun) return;
	const viewer = document.getElementById("viewer");
	if (!viewer.hidden) return closeViewer();
	const el = e.target.closest("[data-nav]");
	if (!el) return;
	// links open themselves; everything else goes through activate
	if (el.tagName === "A") {
		playSound("press");
		return;
	}
	e.preventDefault();
	select(el, false);
	activate(el);
});
document.addEventListener("pointerover", (e) => {
	const el = begun && e.target.closest("[data-nav]");
	if (el && navItems().includes(el)) select(el);
});

loadRelease();
if (AUTO_BEGIN) begin();
// without the world (no WebGL2), controllers are polled on a timer instead of each frame
startWorld()
	.catch((err) => console.warn("menu world unavailable:", err))
	.then((running) => {
		if (!running) setInterval(pollPads, 50);
	});

// >> end << //
