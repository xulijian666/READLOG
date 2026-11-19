use std::path::Path;

const SKILL: &str = include_str!("../skill/SKILL.md");
const INSTALL_PROMPT: &str = include_str!("../skill/install-prompt.md");

fn render(template: &str, cli: &Path) -> String {
    let literal = format!("'{}'", cli.to_string_lossy().replace('\'', "''"));
    template.replacen("{{CLI}}", &literal, 1)
}

pub fn skill_document(cli: &Path) -> String {
    render(SKILL, cli)
}

pub fn install_prompt(cli: &Path) -> String {
    render(INSTALL_PROMPT, cli)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_documents_bind_the_installed_executable_as_a_powershell_literal() {
        let cli = Path::new(r"C:\日志工具\User's $ Tools\readgcislog.exe");
        let binding = r"$cli = 'C:\日志工具\User''s $ Tools\readgcislog.exe'";

        for document in [install_prompt(cli), skill_document(cli)] {
            assert!(document.contains(binding));
            assert!(document.contains("& $cli entries"));
            assert!(!document.contains("{{CLI}}"));
        }
    }
}
