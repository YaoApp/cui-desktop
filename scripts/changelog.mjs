#!/usr/bin/env node
/**
 * 版本 changelog（**一个版本一个文件**，留存于仓库 `changelogs/v<version>.md`）。
 *
 * 内容 = **cui-desktop 的提交** + **CUI 仓库的提交**（发版包会 pin CUI tag，两者都要列）。
 * 正式发版时 CI 只负责「读取该版本文件」作为 GitHub Release 的 note。
 *
 * 用法：
 *   node scripts/changelog.mjs update  --version 1.1.11 [--date 2026-09-29] [--prev v1.1.10]
 *       从 <prev>..HEAD 生成 `changelogs/v1.1.11.md`（已存在则覆盖），并刷新索引
 *   # CUI 侧（可选；缺省自动探测 ../cui 或 ./cui）：
 *       [--cui-dir PATH] [--cui-from v1.1.10 | --cui-since 2026-09-24] [--cui-to HEAD]
 *   node scripts/changelog.mjs extract --version 1.1.11 [--out notes.md]
 *   node scripts/changelog.mjs list
 *
 * 注意：`update` 需要完整 git 历史与 tags（CI 里 actions/checkout 要 fetch-depth: 0）。
 */
import { execFileSync } from 'node:child_process'
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
const DIR = join(ROOT, 'changelogs')
const args = process.argv.slice(2)
const cmd = args[0]
const val = (f, d) => {
	const i = args.indexOf(f)
	return i >= 0 && args[i + 1] ? args[i + 1] : d
}
const gitIn = (dir, a) => execFileSync('git', a, { cwd: dir, encoding: 'utf8' }).trim()
const git = (a) => gitIn(ROOT, a)
const fileFor = (v) => join(DIR, `v${v}.md`)

const GROUPS = [
	['feat', '🚀 Features'],
	['fix', '🐛 Fixes'],
	['perf', '⚡ Performance'],
	['refactor', '♻️ Refactor'],
	['docs', '📝 Docs'],
	['test', '✅ Tests'],
	['build', '📦 Build'],
	['ci', '🤖 CI'],
	['chore', '🧹 Chores'],
	['other', '🔧 Others']
]

function today(d) {
	if (d) return d
	const n = new Date()
	return `${n.getUTCFullYear()}-${String(n.getUTCMonth() + 1).padStart(2, '0')}-${String(n.getUTCDate()).padStart(2, '0')}`
}

/** 收集某个仓库在给定范围内的提交（Conventional Commits 解析） */
function collect(dir, { from, to = 'HEAD', since } = {}) {
	const logArgs = ['log', '--no-merges', '--pretty=format:%s\t%h']
	if (from) logArgs.push(`${from}..${to}`)
	else if (since) logArgs.push(`--since=${since}`, to)
	else logArgs.push(to)
	let rows = []
	try {
		rows = gitIn(dir, logArgs).split('\n').filter(Boolean)
	} catch {
		rows = []
	}
	return rows.map((r) => {
		const [subject = '', sha = ''] = r.split('\t')
		const m = /^([a-z]+)(\([^)]*\))?!?:\s*(.*)$/i.exec(subject)
		return {
			type: m ? m[1].toLowerCase() : 'other',
			scope: m && m[2] ? m[2].replace(/[()]/g, '') : '',
			text: m ? m[3] : subject,
			sha
		}
	})
}

/** 把提交按类型渲染成 markdown 分组（level 为 ### 或 ####） */
function renderGroups(commits, level = '###') {
	const byType = new Map(GROUPS.map(([t]) => [t, []]))
	for (const c of commits) {
		const bucket = byType.has(c.type) ? c.type : 'other'
		byType.get(bucket).push(c)
	}
	const out = []
	let total = 0
	for (const [type, title] of GROUPS) {
		const items = byType.get(type) || []
		if (!items.length) continue
		total += items.length
		out.push(`${level} ${title}`, '')
		for (const c of items) out.push(`- ${c.text} (${c.scope ? c.scope + ', ' : ''}\`${c.sha}\`)`)
		out.push('')
	}
	if (total === 0) out.push('_No notable changes._', '')
	return out
}

function build(version, date, range, prev, cui) {
	const commits = collect(ROOT, { from: prev || undefined })
	const out = [`# v${version} (${date})`, '']
	out.push(prev ? `Since \`${prev}\`.` : 'Initial version.')
	out.push('')
	out.push('## cui-desktop', '')
	out.push(...renderGroups(commits))
	if (cui) {
		out.push(`## CUI ([YaoApp/cui](https://github.com/YaoApp/cui))`, '')
		out.push(`_Range: ${cui.label}_`, '')
		out.push(...renderGroups(cui.commits))
	}
	return out.join('\n').trimEnd() + '\n'
}

function refreshIndex() {
	if (!existsSync(DIR)) return
	const files = readdirSync(DIR)
		.filter((f) => /^v\d+\.\d+\.\d+\.md$/.test(f))
		.sort((a, b) => {
			const pa = a.slice(1, -3).split('.').map(Number)
			const pb = b.slice(1, -3).split('.').map(Number)
			return pb[0] - pa[0] || pb[1] - pa[1] || pb[2] - pa[2]
		})
	const lines = ['# Changelogs', '', 'One file per released version.', '', '| Version | File |', '| --- | --- |']
	for (const f of files) lines.push(`| ${f.slice(1, -3)} | [${f}](${f}) |`)
	writeFileSync(join(DIR, 'README.md'), lines.join('\n') + '\n')
}

function resolveCuiDir(explicit) {
	if (explicit) return existsSync(explicit) ? explicit : null
	for (const c of [join(ROOT, '..', 'cui'), join(ROOT, 'cui')]) {
		if (existsSync(join(c, '.git'))) return c
	}
	return null
}

if (cmd === 'update') {
	const version = val('--version', '')
	if (!version) {
		console.error('update requires --version')
		process.exit(2)
	}
	const date = today(val('--date', ''))
	let prev = val('--prev', '')
	if (!prev && !args.includes('--no-prev')) {
		try {
			prev = git(['describe', '--tags', '--abbrev=0', '--match', 'v[0-9]*'])
		} catch {
			prev = ''
		}
	}
	const range = prev ? `${prev}..HEAD` : 'HEAD'

	// CUI 侧（可选）
	let cui = null
	const cuiDir = resolveCuiDir(val('--cui-dir', ''))
	if (cuiDir && !args.includes('--no-cui')) {
		const cuiFrom = val('--cui-from', '')
		const cuiSince = val('--cui-since', '')
		const cuiTo = val('--cui-to', 'HEAD')
		const commits = collect(cuiDir, { from: cuiFrom || undefined, to: cuiTo, since: cuiSince || undefined })
		const label = cuiFrom ? `${cuiFrom}..${cuiTo}` : cuiSince ? `since ${cuiSince}` : `up to ${cuiTo}`
		cui = { commits, label }
	}

	mkdirSync(DIR, { recursive: true })
	const existed = existsSync(fileFor(version))
	writeFileSync(fileFor(version), build(version, date, range, prev, cui))
	refreshIndex()
	console.log(
		`changelogs/v${version}.md: ${existed ? 'updated' : 'created'} (range ${range}, date ${date})` +
			(cui ? ` + CUI(${cui.commits.length} commits, ${cui.label})` : ' (no CUI section)')
	)
} else if (cmd === 'extract') {
	const version = val('--version', '')
	if (!version) {
		console.error('extract requires --version')
		process.exit(2)
	}
	const f = fileFor(version)
	if (!existsSync(f)) {
		console.error(`ERROR: changelogs/v${version}.md not found`)
		process.exit(1)
	}
	const body = readFileSync(f, 'utf8')
	const out = val('--out', '')
	if (out) writeFileSync(out, body)
	if (!args.includes('--quiet')) process.stdout.write(body)
} else if (cmd === 'list') {
	if (existsSync(DIR)) {
		for (const f of readdirSync(DIR).filter((f) => /^v\d+\.\d+\.\d+\.md$/.test(f)).sort()) {
			console.log(f.slice(1, -3))
		}
	}
} else {
	console.error('usage: changelog.mjs update|extract|list ...')
	process.exit(2)
}
