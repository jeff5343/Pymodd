use std::ops::Add;

use heck::ToPascalCase;

use crate::game_data::{
    directory::{Directory, GameItem},
    entity_types::CategoryToEntityTypes,
    GameData,
};

use super::utils::surround_string_with_quotes;

pub struct MappingFile {}

impl MappingFile {
    pub fn build_content(game_data: &GameData) -> String {
        let game_class_name = game_data.name.to_pascal_case().to_string();
        let mut content = format!(
            "from pymodd.script import Game, Folder, write_game_to_output, write_to_output\n\n\
            from scripts import *\n\
            from entity_scripts import * \n\n\
            class {game_class_name}(Game):\n\
                \tdef _build(self):\n\
                    \t\tself.entity_scripts = [{}]\n\
                    \t\tself.scripts = [\n",
            retrieve_clases_of_entity_scripts(&game_data.entity_type_categories).join(", ")
        );
        content.push_str(
            &build_directory_elements(&game_data.directory)
                .into_iter()
                .map(|element| format!("{}{element}\n", "\t".repeat(3)))
                .collect::<String>()
                .as_str(),
        );
        let project_directory = game_data.project_directory_name();
        content.add(
            &format!(
                "\t\t]\n\n\
                # run `python {project_directory}/mapping.py` to generate this game's files\n\
                write_game_to_output({game_class_name}())\n\
                # uncomment the following to quickly generate the json file for a script\n\
                # write_to_output('output/', SCRIPT_OBJECT())"
            )
            .as_str(),
        )
    }
}

fn retrieve_clases_of_entity_scripts(
    entity_type_categories: &CategoryToEntityTypes,
) -> Vec<String> {
    entity_type_categories
        .iter()
        .flat_map(|(_category, entity_types)| entity_types)
        .filter(|entity_type| !entity_type.directory.is_empty())
        .map(|entity_type| entity_type.class_name().add("()"))
        .collect()
}

fn build_directory_elements(directory: &Directory) -> Vec<String> {
    let mut elements = Vec::new();
    let mut curr_depth = 0;
    directory.into_iter().for_each(|game_item| {
        elements.push(match game_item {
            GameItem::Dir(directory) => {
                curr_depth += 1;
                format!(
                    "{}Folder({}, [",
                    "\t".repeat(curr_depth - 1),
                    surround_string_with_quotes(&directory.name)
                )
            }
            GameItem::Script(script) => {
                format!("{}{}(),", "\t".repeat(curr_depth), script.class_name())
            }
            GameItem::DirectoryEnd => {
                curr_depth -= 1;
                format!("{}]),", "\t".repeat(curr_depth))
            }
        })
    });
    elements
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{
        game_data::{directory::Directory, GameData},
        generator::mapping_file::build_directory_elements,
    };

    use super::MappingFile;

    #[test]
    fn directory_content() {
        assert_eq!(
            build_directory_elements(&Directory::parse(&json!({
                "WI31HDK": { "name": "initialize", "key": "WI31HDK", "actions": [], "parent": None::<&str>, "order": 1},
                "31IAD2B": { "folderName": "utils", "key": "31IAD2B", "parent": None::<&str>, "order": 2 },
                "SDUW31W": { "name": "change_state", "key": "SDUW31W", "actions": [], "parent": "31IAD2B", "order": 1 },
                "Q31E2RS": { "name": "check_players", "key": None::<&str>, "actions": [], "parent": "31IAD2B", "order": 2 },
                "HWI31WQ": { "folderName": "other", "key": "HWI31WQ", "parent": "31IAD2B", "order": 3 },
                "JK32Q03": { "name": "destroy_server", "key": "JK32Q03", "actions": [], "parent": "HWI31WQ", "order": 1},
            }))).into_iter().collect::<String>(),
            String::from(
                "Initialize(),\
                Folder('utils', [\
                    \tChangeState(),\
                    \tCheckPlayers(),\
                    \tFolder('other', [\
                        \t\tDestroyServer(),\
                    \t]),\
                ]),"
            )
        );
    }

    #[test]
    fn simple_mapping_file_content() {
        assert_eq!(MappingFile::build_content(&GameData::parse(r#"{
            "title": "test_game",
            "data": {
                "scripts": {
                    "WI31HDK": { "name": "initialize", "key": "WI31HDK", "actions": [], "parent": null, "order": 1},
                    "31IAD2B": { "folderName": "utils", "key": "31IAD2B", "parent": null, "order": 2 },
                    "SDUW31W": { "name": "change_state", "key": "SDUW31W", "actions": [], "parent": "31IAD2B", "order": 1 }
                },
                "unitTypes": {
                    "RW31QW2": { "name": "bob", "scripts": {
                        "DF31W32": { "name": "initialize", "key": "DF31W32", "actions": [], "parent": null, "order": 1 }
                    }},
                    "IO53IWD": { "name": "empty" }
                }
            }
        }"#.to_string())), 
                   "from pymodd.script import Game, Folder, write_game_to_output, write_to_output\n\n\
                    from scripts import *\n\
                    from entity_scripts import * \n\n\
                    class TestGame(Game):\n\
                        \tdef _build(self):\n\
                            \t\tself.entity_scripts = [Bob()]\n\
                            \t\tself.scripts = [\n\
                                \t\t\tInitialize(),\n\
                                \t\t\tFolder('utils', [\n\
                                    \t\t\t\tChangeState(),\n\
                                \t\t\t]),\n\
                            \t\t]\n\n\
                    # run `python test_game/mapping.py` to generate this game's files\n\
                    write_game_to_output(TestGame())\n\
                    # uncomment the following to quickly generate the json file for a script\n\
                    # write_to_output('output/', SCRIPT_OBJECT())");
    }
}
