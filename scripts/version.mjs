#!/usr/bin/env node
/**
 * 版本助手（cui-desktop）
 *
 * 版本模型：
 *   - `version.json` 的 `version` = **最新稳定版基线**（与最新稳定 tag `vX.Y.Z` 一致）。
 *   - `next`    = 依据 `v<base>..HEAD` 的 Conventional Commits 计算下一个稳定版
 *                 （BREAKING/! → major；含 feat → minor；其余 → patch）。
 *   - `nightly` = `<next>-nightly.<YYYYMMDD>`（SemVer 预发布，不污染稳定号）。
 *
 * 用法：
 *   node scripts/version.mjs current                 # 打印 version.json 的版本
 *   node scripts/version.mjs next                    # 打印下一个稳定版
 *   node scripts/version.mjs next --json             # {"version":"1.1.11","level":"patch","commits":3}
 *   node scripts/version.mjs nightly                 # 1.1.11-nightly.20260929
 *   node scripts/version.mjs nightly --date 20260929 # 指定日期
 *   node scripts/version.mjs bump [--dry-run]        # 把 next 写回 version.json（稳定发版后使用）
 */
import { execFileSync } from 'node:child_process'
import { readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
const VERSION_FILE = join(ROOT, 'version.json')

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

/** 依据 base..HEAD 的提交推断升位级别 */
function analyze(base) {
	let subjects = []
	let bodies = ''
	try {
		subjects = git(['log', '--no-merges', '--pretty=format:%s', `v${base}..HEAD`])
			.split('\n')
			.map((s) => s.trim())
			.filter(Boolean)
		bodies = git(['log', '--no-merges', '--pretty=format:%B', `v${base}..HEAD`])
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
	const base = val('--base', readVersion())
	const { level } = analyze(base)
	const version = `${bump(base, level)}-nightly.${today(val('--date'))}`
	console.log(version)
} else if (cmd === 'bump') {
	const base = readVersion()
	const { level, commits } = analyze(base)
	const version = bump(base, level)
	if (has('--dry-run')) {
		console.log(`${base} -> ${version} (${level}, ${commits} commits)`)
	} else {
		writeFileSync(VERSION_FILE, JSON.stringify({ version }, null, 2) + '\n')
		console.log(`${base} -> ${version}`)
	}
} else {
	console.error(`unknown command: ${cmd}`)
	process.exit(2)
}
