#!/usr/bin/env node
/**
 * 版本 changelog（**一个版本一个文件**，留存于仓库 `changelogs/v<version>.md`）。
 *
 * 正式发版时 CI 只负责「读取该版本文件」作为 GitHub Release 的 note。
 *
 * 用法：
 *   node scripts/changelog.mjs update  --version 1.1.11 [--date 2026-09-29] [--prev v1.1.10]
 *       从 <prev>..HEAD 的 Conventional Commits 生成 `changelogs/v1.1.11.md`
 *       （已存在则覆盖），并刷新 `changelogs/README.md` 索引
 *   node scripts/changelog.mjs extract --version 1.1.11 [--out notes.md]
 *       输出该版本 changelog（供 CI 作为 Release note；找不到则 exit 1）
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
const git = (a) => execFileSync('git', a, { cwd: ROOT, encoding: 'utf8' }).trim()
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

function collect(range) {
	let rows = []
	try {
		rows = git(['log', '--no-merges', '--pretty=format:%s\t%h', range]).split('\n').filter(Boolean)
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

function build(version, date, range, prev) {
	const commits = collect(range)
	const byType = new Map(GROUPS.map(([t]) => [t, []]))
	for (const c of commits) {
		const bucket = byType.has(c.type) ? c.type : 'other'
		byType.get(bucket).push(c)
	}
	const out = [`# v${version} (${date})`, '']
	out.push(prev ? `Since \`${prev}\`.` : 'Initial version.')
	out.push('')
	let total = 0
	for (const [type, title] of GROUPS) {
		const items = byType.get(type) || []
		if (!items.length) continue
		total += items.length
		out.push(`### ${title}`, '')
		for (const c of items) out.push(`- ${c.text} (${c.scope ? c.scope + ', ' : ''}\`${c.sha}\`)`)
		out.push('')
	}
	if (total === 0) out.push('_No notable changes._', '')
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
	mkdirSync(DIR, { recursive: true })
	const existed = existsSync(fileFor(version))
	writeFileSync(fileFor(version), build(version, date, range, prev))
	refreshIndex()
	console.log(`changelogs/v${version}.md: ${existed ? 'updated' : 'created'} (range ${range}, date ${date})`)
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
