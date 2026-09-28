#!/usr/bin/env node
/**
 * 版本助手（cui-desktop）
 *
 * 版本来源：**单一来源为 cui 仓库根目录的 `version.json`**。
 *   - 路径优先级：`CUI_VERSION_FILE` 环境变量 > `<cui-desktop>/cui/version.json`
 *     （`scripts/pull-cui.sh` 的 checkout）> `<cui-desktop>/../cui/version.json`（同级 checkout）。
 *   - CI 里 `CUI_VERSION_FILE` 指向从 cui main 下载的临时文件。
 *   - cui-desktop 不再保留本地 version.json；`bump` 会写回上面解析到的同一个文件。
 *
 * 版本模型（version.json = **当前正在开发的版本**）：
 *   - 平时不动：一个小版本下面可以积累 N 个 commit，版本号保持不变。
 *   - 正式发版：用 `current`（= 待发布版本）打 tag 并发布；
 *               发布完成后 `bump patch` 切到下一个补丁版（如 1.1.11 → 1.1.12）。
 *   - 开发中出现**破坏性变更**：`bump minor`（如 1.1.x → 1.2.0）。
 *   - `nightly` = `<current>-nightly.<YYYYMMDD>`（与 version.json 一致，不自动升位）。
 *   - `next`    = 按 Conventional Commits 给出的**升位建议**（仅参考，不写文件）。
 *
 * 用法：
 *   node scripts/version.mjs current                  # 当前开发/待发布版本
 *   node scripts/version.mjs nightly                  # 1.1.11-nightly.20260929
 *   node scripts/version.mjs nightly --date 20260929  # 指定日期
 *   node scripts/version.mjs next --json              # 建议升位（仅参考）
 *   node scripts/version.mjs bump patch [--dry-run]   # 发版后：切下一个补丁版
 *   node scripts/version.mjs bump minor               # 破坏性变更：升 minor
 */
import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
// Single source of truth: the cui repo's version.json.
// Resolution order: CUI_VERSION_FILE > <repo>/cui/version.json (pull-cui.sh checkout)
// > <repo>/../cui/version.json (sibling checkout, e.g. local dev workspace).
function resolveVersionFile() {
	if (process.env.CUI_VERSION_FILE) return process.env.CUI_VERSION_FILE
	const candidates = [join(ROOT, 'cui', 'version.json'), join(ROOT, '..', 'cui', 'version.json')]
	for (const f of candidates) {
		if (existsSync(f)) return f
	}
	return candidates[0]
}
const VERSION_FILE = resolveVersionFile()

const args = process.argv.slice(2)
const cmd = args[0]
const has = (f) => args.includes(f)
const val = (f, d) => {
	const i = args.indexOf(f)
	return i >= 0 && args[i + 1] ? args[i + 1] : d
}

function git(a) {
	return execFileSync('git', a, { cwd: ROOT, encoding: 'utf8' }).trim()
}

function readVersion() {
	if (!existsSync(VERSION_FILE)) {
		console.error(`ERROR: version file not found: ${VERSION_FILE}`)
		console.error('Run `bash scripts/pull-cui.sh` to fetch the cui repo, or set CUI_VERSION_FILE to the path of a cui version.json.')
		process.exit(1)
	}
	return JSON.parse(readFileSync(VERSION_FILE, 'utf8')).version
}

function parse(v) {
	const m = /^(\d+)\.(\d+)\.(\d+)/.exec(String(v || ''))
	if (!m) throw new Error(`invalid version: ${v}`)
	return { major: +m[1], minor: +m[2], patch: +m[3] }
}

function bump(v, level) {
	const s = parse(v)
	if (level === 'major') return `${s.major + 1}.0.0`
	if (level === 'minor') return `${s.major}.${s.minor + 1}.0`
	return `${s.major}.${s.minor}.${s.patch + 1}`
}

/** 解析提交范围起点：优先 `v<base>`；若该 tag 不存在（version.json 已提前 bump），退回最近的可达 tag */
function rangeFrom(base) {
	const tag = `v${base}`
	try {
		git(['rev-parse', '--verify', '--quiet', `${tag}^{commit}`])
		return tag
	} catch {
		try {
			return git(['describe', '--tags', '--abbrev=0', '--match', 'v[0-9]*'])
		} catch {
			return null
		}
	}
}

/** 依据 from..HEAD 的提交推断升位级别 */
function analyze(base) {
	const from = rangeFrom(base)
	if (!from) return { level: 'patch', commits: 0, base }
	const range = `${from}..HEAD`
	let subjects = []
	let bodies = ''
	try {
		subjects = git(['log', '--no-merges', '--pretty=format:%s', range])
			.split('\n')
			.map((s) => s.trim())
			.filter(Boolean)
		bodies = git(['log', '--no-merges', '--pretty=format:%B', range])
	} catch {
		return { level: 'patch', commits: 0, base }
	}
	if (subjects.length === 0) return { level: 'patch', commits: 0, base }
	let level = 'patch'
	if (/\bBREAKING[ -]CHANGE\b/i.test(bodies) || subjects.some((s) => /^[a-z]+(\([^)]*\))?!:/i.test(s))) {
		level = 'major'
	} else if (subjects.some((s) => /^feat(\([^)]*\))?:/i.test(s))) {
		level = 'minor'
	}
	return { level, commits: subjects.length, base }
}

function today(d) {
	if (d) return d
	const n = new Date()
	return `${n.getUTCFullYear()}${String(n.getUTCMonth() + 1).padStart(2, '0')}${String(n.getUTCDate()).padStart(2, '0')}`
}

if (!cmd || cmd === 'help' || cmd === '--help') {
	console.log(readFileSync(fileURLToPath(import.meta.url), 'utf8').split('*/')[0].replace(/^\/\*\*?/, '').trim())
	process.exit(0)
}

if (cmd === 'current') {
	console.log(readVersion())
} else if (cmd === 'next') {
	const base = val('--base', readVersion())
	const { level, commits } = analyze(base)
	const version = bump(base, level)
	if (has('--json')) console.log(JSON.stringify({ version, level, commits, base }))
	else console.log(version)
} else if (cmd === 'nightly') {
	// 约定：nightly = <base>-nightly.<date>，base 直接取自共享 version.json（不做自动升位）。
	const base = val('--base', readVersion())
	const version = `${base}-nightly.${today(val('--date'))}`
	console.log(version)
} else if (cmd === 'bump') {
	// 显式级别：bump patch|minor|major（不指定则按提交推断，仅作便利）
	const explicit = args[1]
	const LEVELS = ['patch', 'minor', 'major']
	if (explicit && !LEVELS.includes(explicit)) {
		console.error(`invalid level: ${explicit} (expected: patch|minor|major)`)
		process.exit(2)
	}
	const base = readVersion()
	const { level, commits } = analyze(base)
	const finalLevel = explicit || level
	const version = bump(base, finalLevel)
	if (has('--dry-run')) {
		console.log(
			`${base} -> ${version} (${finalLevel}${explicit ? ', explicit' : `, suggested from ${commits} commits`})`
		)
	} else {
		writeFileSync(VERSION_FILE, JSON.stringify({ version }, null, 2) + '\n')
		console.log(`${base} -> ${version}`)
	}
} else {
	console.error(`unknown command: ${cmd}`)
	process.exit(2)
}
