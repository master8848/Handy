use crate::cli::{PromptCmd, PromptSortArg, SkillCmd};
use crate::managers::prompt_library::{Prompt, PromptFilter, PromptLibraryManager, PromptSort};
use anyhow::Result;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;

/// Headless prompt/skill CLI dispatched from `lib.rs` setup worker. All output
/// goes to stdout (JSON or plain); logs stay on stderr so `| jq` keeps working.

fn sort_arg_to_sort(arg: &Option<PromptSortArg>) -> PromptSort {
    match arg {
        Some(PromptSortArg::UpdatedDesc) => PromptSort::UpdatedDesc,
        Some(PromptSortArg::CreatedDesc) => PromptSort::CreatedDesc,
        Some(PromptSortArg::UsageDesc) => PromptSort::UsageDesc,
        Some(PromptSortArg::TitleAsc) => PromptSort::TitleAsc,
        None => PromptSort::UpdatedDesc,
    }
}

fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in title.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            prev_dash = false;
        } else if !prev_dash && !slug.is_empty() {
            slug.push('-');
            prev_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "prompt".to_string()
    } else {
        // agentskills spec: ^[a-z0-9]+(-[a-z0-9]+)*$  1..64 chars
        let s = if slug.len() > 64 { slug[..64].trim_matches('-').to_string() } else { slug };
        if s.is_empty() { "prompt".to_string() } else { s }
    }
}

fn prompt_to_skill_md(prompt: &Prompt) -> String {
    let name = slugify(&prompt.title);
    let desc = if prompt.title.len() > 200 {
        prompt.title[..200].to_string()
    } else if prompt.content.len() > 200 {
        format!("{} — {}", prompt.title, &prompt.content[..200.min(prompt.content.len())].replace('\n', " "))
    } else {
        prompt.title.clone()
    };
    let desc = desc.replace('"', "\\\"").replace('\n', " ");
    let vars_note = if prompt.variables.is_empty() {
        String::new()
    } else {
        format!("\n\n## Variables\n\nThis prompt uses variables: {}.\nPass them with `--var k=v` (`handy prompt use`). In SKILL.md they map to `$ARGUMENTS`.\n", prompt.variables.iter().map(|v| format!("`{{{{{}}}}}`", v)).collect::<Vec<_>>().join(", "))
    };
    let tags_line = if prompt.tags.is_empty() { String::new() } else { format!("tags: [{}]\n", prompt.tags.join(", ")) };
    format!(
        "---\nname: {name}\ndescription: \"{desc}\"\n{tags_line}---\n\n# {title}\n\n{content}{vars_note}\n",
        name = name,
        desc = desc,
        tags_line = tags_line,
        title = prompt.title,
        content = prompt.content,
        vars_note = vars_note,
    )
}

fn render_content(content: &str, vars: &[(String, String)]) -> String {
    if vars.is_empty() {
        return content.to_string();
    }
    let map: HashMap<String, String> = vars.iter().cloned().collect();
    crate::managers::prompt_library::render_prompt_content(content, &map)
}

fn print_prompts_json(prompts: &[Prompt]) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(prompts)?);
    Ok(())
}

fn print_prompt_json(prompt: &Prompt) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(prompt)?);
    Ok(())
}

fn resolve_folder_id(_mgr: &PromptLibraryManager, id: Option<i64>) -> Option<i64> {
    id
}

pub fn run_prompt_cmd(mgr: &PromptLibraryManager, cmd: PromptCmd) -> i32 {
    match cmd {
        PromptCmd::Search { query, tag, folder, limit, json, copy: _, raw } => {
            let lim = (limit as i64).clamp(1, 50);
            let prompts = if raw {
                // raw LIKE path
                mgr.list_prompts(PromptFilter {
                    search: Some(query.clone()),
                    folder_id: folder,
                    tag: tag.clone(),
                    sort: Some(PromptSort::UpdatedDesc),
                    limit: Some(lim),
                    ..Default::default()
                })
            } else {
                mgr.search_prompts(&query, folder, Some(lim))
            };
            match prompts {
                Ok(mut prompts) => {
                    if let Some(t) = tag.clone() {
                        if !raw {
                            // search_prompts already excludes tag; apply client filter
                            prompts.retain(|p| p.tags.iter().any(|x| x.eq_ignore_ascii_case(&t)));
                        }
                    }
                    if json {
                        if let Err(e) = print_prompts_json(&prompts) {
                            eprintln!("error: {}", e);
                            return 1;
                        }
                    } else if prompts.is_empty() {
                        println!("No prompts match \"{}\"", query);
                    } else {
                        for p in &prompts {
                            let tags = if p.tags.is_empty() { String::new() } else { format!(" #{}", p.tags.join(" #")) };
                            let folder = p.folder_name.as_deref().unwrap_or("-");
                            println!("{:>4}  {:<30}  [{:<12}] {}  v{}  {} uses{}", p.id, truncate(&p.title, 30), folder, if p.pinned { "★" } else { " " }, p.version, p.usage_count, tags);
                        }
                    }
                    0
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    1
                }
            }
        }
        PromptCmd::List { tag, folder, pinned, sort, limit, offset, json } => {
            let lim = (limit as i64).clamp(1, 100);
            let filter = PromptFilter {
                folder_id: folder,
                tag: tag.clone(),
                pinned_only: if pinned { Some(true) } else { None },
                sort: Some(sort_arg_to_sort(&sort)),
                limit: Some(lim),
                offset,
                ..Default::default()
            };
            match mgr.list_prompts(filter) {
                Ok(prompts) => {
                    if json {
                        if let Err(e) = print_prompts_json(&prompts) {
                            eprintln!("error: {}", e);
                            return 1;
                        }
                    } else if prompts.is_empty() {
                        println!("No prompts.");
                    } else {
                        for p in &prompts {
                            let tags = if p.tags.is_empty() { String::new() } else { format!(" #{}", p.tags.join(" #")) };
                            let folder = p.folder_name.as_deref().unwrap_or("-");
                            println!("{:>4}  {:<30}  [{:<12}] {} v{}  {} uses{}", p.id, truncate(&p.title, 30), folder, if p.pinned { "★" } else { " " }, p.version, p.usage_count, tags);
                        }
                    }
                    0
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    1
                }
            }
        }
        PromptCmd::Get { id, json, raw, var, stdout: _ } => {
            match mgr.get_prompt(id) {
                Ok(p) => {
                    let content = if raw { p.content.clone() } else { render_content(&p.content, &var) };
                    if json {
                        // emit the Prompt shape with rendered content when vars given
                        let mut out = p.clone();
                        if !raw && !var.is_empty() {
                            out.content = content.clone();
                        }
                        if let Err(e) = print_prompt_json(&out) {
                            eprintln!("error: {}", e);
                            return 1;
                        }
                    } else {
                        // stdout is the content
                        println!("{}", content);
                    }
                    0
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    1
                }
            }
        }
        PromptCmd::Use { query, id, var, stdout: _, copy: _, json } => {
            let prompt_res = if let Some(pid) = id {
                mgr.get_prompt(pid)
            } else if let Ok(n) = query.parse::<i64>() {
                // numeric query without --id → try id first
                match mgr.get_prompt(n) {
                    Ok(p) => Ok(p),
                    Err(_) => {
                        // fall back to search
                        let v = mgr.search_prompts(&query, None, Some(1)).unwrap_or_default();
                        v.into_iter().next().ok_or_else(|| anyhow::anyhow!("no prompt matching \"{}\"", query))
                    }
                }
            } else {
                let v = mgr.search_prompts(&query, None, Some(1)).unwrap_or_default();
                v.into_iter().next().ok_or_else(|| anyhow::anyhow!("no prompt matching \"{}\"", query))
            };
            match prompt_res {
                Ok(p) => {
                    let rendered = render_content(&p.content, &var);
                    if rendered.contains("{{") {
                        eprintln!("error: prompt still contains unfilled variables: {:?}", p.variables);
                        return 2;
                    }
                    if json {
                        let mut out = p.clone();
                        out.content = rendered.clone();
                        if let Err(e) = print_prompt_json(&out) {
                            eprintln!("error: {}", e);
                            return 1;
                        }
                    } else {
                        println!("{}", rendered);
                    }
                    let _ = mgr.increment_usage(p.id);
                    0
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    1
                }
            }
        }
        PromptCmd::Create { title, content, folder, tags, json } => {
            let mut text = content.clone();
            if text.is_none() {
                // read stdin
                let mut buf = String::new();
                if std::io::stdin().read_to_string(&mut buf).is_ok() {
                    let t = buf.trim().to_string();
                    if !t.is_empty() {
                        text = Some(t);
                    }
                }
            }
            let content_val = match text {
                Some(c) if !c.trim().is_empty() => c,
                _ => {
                    eprintln!("error: --content is required (or pipe content on stdin)");
                    return 2;
                }
            };
            let fid = resolve_folder_id(mgr, folder);
            match mgr.create_prompt(title.clone(), content_val, fid, tags.clone()) {
                Ok(p) => {
                    if json {
                        if let Err(e) = print_prompt_json(&p) {
                            eprintln!("error: {}", e);
                            return 1;
                        }
                    } else {
                        println!("Created prompt {} \"{}\"", p.id, p.title);
                    }
                    0
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    1
                }
            }
        }
    }
}

pub fn run_skill_cmd(mgr: &PromptLibraryManager, cmd: SkillCmd) -> i32 {
    match cmd {
        SkillCmd::Export { id, format: _, out, stdout } => {
            match mgr.get_prompt(id) {
                Ok(p) => {
                    let md = prompt_to_skill_md(&p);
                    if stdout || out.is_none() {
                        print!("{}", md);
                        let _ = std::io::stdout().flush();
                    }
                    if let Some(dir) = out {
                        let need_dir = if dir.extension().is_some() { dir.parent().map(|p| p.to_path_buf()).unwrap_or(PathBuf::from(".")) } else { dir.clone() };
                        let target_dir = if dir.extension().is_some() { need_dir } else { dir };
                        if let Err(e) = std::fs::create_dir_all(&target_dir) {
                            eprintln!("error: create dir {}: {}", target_dir.display(), e);
                            return 1;
                        }
                        let path = target_dir.join("SKILL.md");
                        match std::fs::write(&path, md) {
                            Ok(_) => eprintln!("Wrote {}", path.display()),
                            Err(e) => {
                                eprintln!("error: write {}: {}", path.display(), e);
                                return 1;
                            }
                        }
                    }
                    0
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    1
                }
            }
        }
        SkillCmd::Add { source, folder, tags } => {
            let path = PathBuf::from(&source);
            let text = if path.exists() {
                // if directory, look for SKILL.md
                let file = if path.is_dir() { path.join("SKILL.md") } else { path.clone() };
                match std::fs::read_to_string(&file) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("error: read {}: {}", file.display(), e);
                        return 1;
                    }
                }
            } else {
                // treat source as raw markdown content (e.g. URL not supported offline)
                eprintln!("error: file not found: {}", source);
                return 2;
            };
            // Parse SKILL.md frontmatter
            let (title, content) = parse_skill_md(&text);
            let title = if title.trim().is_empty() { "Imported Skill".to_string() } else { title };
            let content = if content.trim().is_empty() { text } else { content };
            let all_tags = tags.clone();
            // keep existing skill tags if present in frontmatter
            match mgr.create_prompt(title.clone(), content, folder, all_tags) {
                Ok(p) => {
                    println!("Imported prompt {} \"{}\"", p.id, p.title);
                    0
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    1
                }
            }
        }
        SkillCmd::Find { query, json, limit } => {
            let q = query.unwrap_or_default();
            let prompts = if q.trim().is_empty() {
                mgr.list_prompts(PromptFilter { limit: Some(limit as i64), ..Default::default() }).unwrap_or_default()
            } else {
                mgr.search_prompts(&q, None, Some(limit as i64)).unwrap_or_default()
            };
            if json {
                if let Err(e) = print_prompts_json(&prompts) {
                    eprintln!("error: {}", e);
                    return 1;
                }
            } else if prompts.is_empty() {
                println!("No prompts match \"{}\"", q);
            } else {
                for p in &prompts {
                    println!("{:>4}  {}", p.id, p.title);
                }
            }
            0
        }
    }
}

fn parse_skill_md(text: &str) -> (String, String) {
    let mut title = String::new();
    let mut body = text.to_string();
    // very small frontmatter parser: ---\n ... \n---\n
    if text.starts_with("---") {
        if let Some(end) = text[3..].find("\n---") {
            let fm = &text[3..3 + end];
            let rest = &text[3 + end + 4..];
            for line in fm.lines() {
                let line = line.trim();
                if let Some(v) = line.strip_prefix("name:") {
                    if title.is_empty() {
                        title = v.trim().trim_matches('"').trim().to_string();
                    }
                }
                if let Some(v) = line.strip_prefix("description:") {
                    if title.is_empty() {
                        title = v.trim().trim_matches('"').trim().to_string();
                    }
                }
            }
            // body after frontmatter, skip leading # title
            let mut r = rest.trim_start().to_string();
            if r.starts_with('#') {
                if let Some(nl) = r.find('\n') {
                    if title.is_empty() {
                        title = r[1..nl].trim().to_string();
                    }
                    r = r[nl + 1..].trim_start().to_string();
                }
            }
            body = r;
            if title.is_empty() {
                // fallback to first heading
            }
        }
    } else if text.starts_with('#') {
        if let Some(nl) = text.find('\n') {
            title = text[1..nl].trim().to_string();
            body = text[nl + 1..].trim_start().to_string();
        }
    }
    // If title still empty, use first line
    if title.is_empty() {
        if let Some(nl) = body.find('\n') {
            title = body[..nl].trim().to_string();
            if title.len() > 80 { title.truncate(80); }
        } else if !body.is_empty() {
            title = body[..80.min(body.len())].trim().to_string();
        }
    }
    (title, body)
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n { s.to_string() } else { format!("{}…", &s[..n - 1]) }
}
