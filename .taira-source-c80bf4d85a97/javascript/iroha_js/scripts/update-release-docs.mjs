#!/usr/bin/env node

/**
 * Update project documentation for an @iroha/iroha-js release.
 *
 * - Prepends the release notes to CHANGELOG.md
 * - Inserts a new "Latest Updates" section into status.md with the release summary
 * - Appends an entry to the JavaScript roadmap progress table and marks the
 *   JS5 release-notes automation task as complete.
 *
 * Usage:
 *   node ./scripts/update-release-docs.mjs --version 0.1.0 \
 *     --date 2026-02-01 \
 *     --note "Added Torii Connect session helpers" \
 *     --note "Documented release workflow automation"
 *
 * Notes can also be provided via --notes-file (<path>). Multiple --note flags
 * are concatenated; blank lines and leading "-" characters are stripped.
 */

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

function fail(message) {
  console.error(message);
  process.exit(1);
}

function parseArgs(argv) {
  const args = {
    notes: [],
    notesFile: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    switch (arg) {
      case "--version": {
        index += 1;
        args.version = argv[index];
        break;
      }
      case "--date": {
        index += 1;
        args.date = argv[index];
        break;
      }
      case "--note": {
        index += 1;
        args.notes.push(argv[index]);
        break;
      }
      case "--notes-file": {
        index += 1;
        args.notesFile = argv[index];
        break;
      }
      default: {
        fail(`Unknown argument: ${arg}`);
      }
    }
  }
  if (!args.version) {
    fail("Missing required --version argument");
  }
  return args;
}

function normaliseDate(inputDate) {
  if (!inputDate) {
    return new Date().toISOString().slice(0, 10);
  }
  if (!/^\d{4}-\d{2}-\d{2}$/.test(inputDate)) {
    fail(`Invalid date format "${inputDate}". Use YYYY-MM-DD.`);
  }
  return inputDate;
}

function humanReadableDate(date) {
  const [yearStr, monthStr, dayStr] = date.split("-");
  const months = [
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "May",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec",
  ];
  const monthIndex = Number.parseInt(monthStr, 10) - 1;
  if (monthIndex < 0 || monthIndex >= months.length) {
    fail(`Invalid month in date "${date}"`);
  }
  const day = Number.parseInt(dayStr, 10);
  return `${months[monthIndex]} ${day}, ${yearStr}`;
}

function loadNotes(notes, notesFile) {
  const collected = [];
  for (const note of notes) {
    if (!note) {
      continue;
    }
    const trimmed = note.replace(/^[\s*-]+/, "").trim();
    if (trimmed.length > 0) {
      collected.push(trimmed);
    }
  }
  if (notesFile) {
    const fileContent = fs.readFileSync(notesFile, "utf8");
    for (const line of fileContent.split(/\r?\n/)) {
      const trimmed = line.replace(/^[\s*-]+/, "").trim();
      if (trimmed.length > 0) {
        collected.push(trimmed);
      }
    }
  }
  if (collected.length === 0) {
    fail("At least one --note or --notes-file entry is required");
  }
  return collected;
}

function updateChangelog(changelogPath, version, date, notes) {
  const changelog = fs.readFileSync(changelogPath, "utf8");
  if (changelog.includes(`## [${version}]`)) {
    fail(`CHANGELOG.md already contains an entry for version ${version}`);
  }
  const sectionLines = [`## [${version}] - ${date}`, ""];
  for (const note of notes) {
    sectionLines.push(`- ${note}`);
  }
  sectionLines.push("");
  const sectionText = `${sectionLines.join("\n")}\n`;
  const firstHeadingIndex = changelog.indexOf("## [");
  const updated =
    firstHeadingIndex === -1
      ? `${changelog.trimEnd()}\n\n${sectionText}`
      : `${changelog.slice(0, firstHeadingIndex)}${sectionText}${changelog.slice(firstHeadingIndex)}`;
  fs.writeFileSync(changelogPath, updated);
  const lines = updated.split("\n");
  const headingLine = lines.findIndex((line) => line.startsWith(`## [${version}]`)) + 1;
  return headingLine;
}

function updateStatus(statusPath, dateIso, version, notes, changelogLine) {
  const statusContent = fs.readFileSync(statusPath, "utf8");
  const humanDate = humanReadableDate(dateIso);
  const latestHeading = `## Latest Updates (${humanDate})`;
  if (statusContent.includes(latestHeading)) {
    fail(`status.md already contains a section for ${humanDate}`);
  }
  const joinedNotes = notes.join("; ");
  const releaseSummary =
    `- **@iroha/iroha-js v${version} release.** ${joinedNotes} ` +
    `Release notes recorded in the package changelog.${createReference(
      `javascript/iroha_js/CHANGELOG.md#L${changelogLine}`,
    )}${createReference("javascript/iroha_js/scripts/update-release-docs.mjs#L1")}`;
  const updatedDate = statusContent.replace(
    /Last updated: .*/,
    `Last updated: ${dateIso}`,
  );
  const latestIndex = updatedDate.indexOf("\n## Latest Updates");
  if (latestIndex === -1) {
    fail("status.md does not contain a \"## Latest Updates\" section to anchor insertions");
  }
  const insertion = `${latestHeading}\n\n${releaseSummary}\n\n`;
  const withSection =
    updatedDate.slice(0, latestIndex + 1) + insertion + updatedDate.slice(latestIndex + 1);
  fs.writeFileSync(statusPath, withSection);
}

function createReference(target) {
  return `【F:${target}】`;
}

function markRoadmapTask(roadmapContent, version, changelogLine) {
  const taskLine =
    "- ⬜ Integrate JS SDK release notes into `CHANGELOG.md` and automate status/roadmap updates per release.";
  if (roadmapContent.includes(taskLine)) {
    const replacement =
      "- ✅ Integrate JS SDK release notes into `CHANGELOG.md` and automate status/roadmap updates per release." +
      createReference(`javascript/iroha_js/scripts/update-release-docs.mjs#L1`) +
      createReference(`javascript/iroha_js/CHANGELOG.md#L${changelogLine}`);
    return roadmapContent.replace(taskLine, replacement);
  }
  return roadmapContent;
}

function updateRoadmap(roadmapPath, version, changelogLine) {
  let content = fs.readFileSync(roadmapPath, "utf8");
  content = markRoadmapTask(content, version, changelogLine);

  const progressHeading = "**JavaScript Progress Since Last Update**";
  const headingIndex = content.indexOf(progressHeading);
  if (headingIndex === -1) {
    fail("roadmap.md is missing the JavaScript progress table");
  }
  const tableHeader = "| Item | Milestone | Status | Notes |";
  const tableStart = content.indexOf(tableHeader, headingIndex);
  if (tableStart === -1) {
    fail("Unable to locate JavaScript progress table header");
  }
  const tableEnd = content.indexOf("\n\n", tableStart);
  const sliceEnd = tableEnd === -1 ? content.length : tableEnd;
  const tableSection = content.slice(tableStart, sliceEnd);
  const tableLines = tableSection.trimEnd().split("\n");

  const newRow =
    `| Release docs automation (${version}) | JS5 | ✅ Complete | ` +
    `Added a release helper to synchronise changelog, status, and roadmap entries for @iroha/iroha-js.${createReference(
      "javascript/iroha_js/scripts/update-release-docs.mjs#L1",
    )}${createReference(`javascript/iroha_js/CHANGELOG.md#L${changelogLine}`)} |`;

  const existingRow = tableLines.some((line) => line.includes("Release docs automation"));
  if (!existingRow) {
    tableLines.splice(2, 0, newRow);
  }

  const updatedTable = tableLines.join("\n");
  const updatedContent =
    content.slice(0, tableStart) +
    updatedTable +
    content.slice(sliceEnd);
  fs.writeFileSync(roadmapPath, updatedContent);
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const version = args.version;
  const dateIso = normaliseDate(args.date);
  const notes = loadNotes(args.notes, args.notesFile);

  const scriptPath = fileURLToPath(import.meta.url);
  const scriptDir = path.dirname(scriptPath);
  const repoRoot = path.resolve(scriptDir, "..", "..", "..");
  const changelogPath = path.resolve(scriptDir, "..", "CHANGELOG.md");
  const statusPath = path.resolve(repoRoot, "status.md");
  const roadmapPath = path.resolve(repoRoot, "roadmap.md");

  const changelogLine = updateChangelog(changelogPath, version, dateIso, notes);
  updateStatus(statusPath, dateIso, version, notes, changelogLine);
  updateRoadmap(roadmapPath, version, changelogLine);

  console.log(
    `Updated release documentation for @iroha/iroha-js v${version} (${dateIso}).`,
  );
}

main();
