use std::ops::Add;

use serde_json::Value;

use crate::game_data::{
    actions::Action,
    directory::{Directory, Script},
    variable_categories::CategoriesToVariables,
    GameData,
};

use super::{
    game_variables_file::pymodd_class_name_of_category,
    utils::{
        iterators::{
            argument_values_iterator::{ArgumentValueIterItem, ArgumentValuesIterator, Operation},
            directory_iterator::DirectoryIterItem,
        },
        surround_string_with_quotes,
    },
};

pub struct ScriptsFile {}

impl ScriptsFile {
    pub fn build_content(game_data: &GameData) -> String {
        format!(
            "from pymodd.actions import *\n\
            from pymodd.functions import *\n\
            from pymodd.script import Trigger, UiTarget, Flip, script\n\n\
            from game_variables import *\n\n\n\
            {}\n\n",
            &build_directory_content(
                &game_data.root_directory,
                &ScriptsContentBuilder::new(
                    &game_data.categories_to_variables,
                    &game_data.root_directory,
                ),
            )
        )
    }
}

pub fn build_directory_content(
    directory: &Directory,
    scripts_class_content_builder: &ScriptsContentBuilder,
) -> String {
    directory
        .iter_flattened()
        .map(|game_item| match game_item {
            DirectoryIterItem::StartOfDirectory(directory) => format!(
                "# ╭\n\
                 # {}\n\
                 # |\n\n",
                directory.name.to_uppercase()
            ),
            DirectoryIterItem::Script(script) => scripts_class_content_builder
                .build_script_content(&script)
                .add("\n\n"),
            DirectoryIterItem::DirectoryEnd => String::from(
                "# |\n\
                 # ╰\n\n",
            ),
        })
        .collect::<String>()
        .trim_end()
        .to_string()
}

pub struct ScriptsContentBuilder<'a> {
    categories_to_variables: &'a CategoriesToVariables,
    root_directory: &'a Directory,
}

impl<'a> ScriptsContentBuilder<'a> {
    pub fn new(
        categories_to_variables: &'a CategoriesToVariables,
        root_directory: &'a Directory,
    ) -> ScriptsContentBuilder<'a> {
        ScriptsContentBuilder {
            categories_to_variables,
            root_directory,
        }
    }

    pub fn build_script_content(&self, script: &Script) -> String {
        let class_name = script.pymodd_class_name();
        format!(
            "@script(triggers=[{}]{})\n\
            class {class_name}():\n\
            \tdef _build(self):\n\
                {}",
            script.triggers_into_pymodd_enums().join(", "),
            if script.name.is_ascii() {
                String::new()
            } else {
                format!(", name={}", surround_string_with_quotes(&script.name))
            },
            if script.actions.len() > 0 {
                self.build_actions_content(&script.actions)
                    .lines()
                    .map(|action| format!("{}{action}\n", "\t".repeat(2)))
                    .collect::<String>()
            } else {
                String::from("\t\tpass\n")
            }
        )
    }

    fn build_actions_content(&self, actions: &Vec<Action>) -> String {
        actions
            .iter()
            .map(|action| self.build_action_content(&action))
            .collect::<String>()
    }

    fn build_action_content(&self, action: &Action) -> String {
        match action.name.as_str() {
            // convert condition actions into if statements
            "condition" => {
                let mut args_iter = action.iter_flattened_argument_values();
                let err_msg = "condition action does not contain valid args";
                let condition = self.build_argument_content(args_iter.next().expect(err_msg));
                let then_actions = self.build_argument_content(args_iter.next().expect(err_msg));
                let else_actions = self.build_argument_content(args_iter.next().expect(err_msg));
                format!(
                    "if {condition}:{}{}",
                    then_actions.strip_suffix("\n").unwrap(),
                    if else_actions != "\n\tpass\n" {
                        format!("\nelse:{else_actions}")
                    } else {
                        String::from("\n")
                    }
                )
            }
            // convert variable for loop actions into for loops
            "for" => {
                let mut args_iter = action.iter_flattened_argument_values();
                let err_msg = "variable for loop action does not contain valid args";
                let variable = self.build_argument_content(args_iter.next().expect(err_msg));
                let start = self.build_argument_content(args_iter.next().expect(err_msg));
                let stop = self.build_argument_content(args_iter.next().expect(err_msg));
                let actions = self.build_argument_content(args_iter.next().expect(err_msg));
                format!("for {variable} in range({start}, {stop}):{actions}")
            }
            // convert repeat actions into for loops
            "repeat" => {
                let mut args_iter = action.iter_flattened_argument_values();
                let err_msg = "repeat action does not contain valid args";
                let count = self.build_argument_content(args_iter.next().expect(err_msg));
                let actions = self.build_argument_content(args_iter.next().expect(err_msg));
                format!("for _ in repeat({count}):{actions}")
            }
            "comment" => {
                format!(
                    "{}({}{})\n",
                    action.pymodd_class_name(),
                    // set argument manually for comments
                    surround_string_with_quotes(
                        action.comment.as_ref().unwrap_or(&String::from("None"))
                    ),
                    self.build_optional_arguments_contents(&action)
                        .into_iter()
                        .skip(1) // skip over optional comment argument
                        .map(|arg| String::from(", ") + &arg)
                        .collect::<String>(),
                )
            }
            _ => format!(
                "{}({}{})\n",
                action.pymodd_class_name(),
                self.build_arguments_content(action.iter_flattened_argument_values()),
                &self
                    .build_optional_arguments_contents(&action)
                    .into_iter()
                    .enumerate()
                    .map(|(i, arg)| {
                        if action.args.is_empty() && i == 0 {
                            arg
                        } else {
                            String::from(", ") + &arg
                        }
                    })
                    .collect::<String>(),
            ),
        }
    }

    fn build_arguments_content(&self, args_iter: ArgumentValuesIterator) -> String {
        args_iter
            .fold(String::from("("), |pymodd_args, arg| {
                let include_seperator =
                    !pymodd_args.ends_with("(") && arg != ArgumentValueIterItem::FunctionEnd;
                pymodd_args
                    + &format!(
                        "{}{}",
                        String::from(if include_seperator { ", " } else { "" }),
                        match arg {
                            // surround entire condition with parenthesis
                            ArgumentValueIterItem::Condition(_) =>
                                format!("({})", self.build_argument_content(arg)),
                            _ => self.build_argument_content(arg),
                        }
                    )
            })
            .strip_prefix("(")
            .unwrap()
            .to_string()
    }

    fn build_argument_content(&self, arg: ArgumentValueIterItem) -> String {
        match arg {
            ArgumentValueIterItem::StartOfFunction(function) => {
                format!("{}(", function.pymodd_class_name())
            }
            ArgumentValueIterItem::Actions(actions) => {
                format!(
                    "\n{}",
                    if actions.len() > 0 {
                        self.build_actions_content(actions)
                            .lines()
                            .map(|line| format!("\t{line}\n"))
                            .collect::<String>()
                    } else {
                        String::from("\tpass\n")
                    }
                )
            }
            ArgumentValueIterItem::Value(value) => match value {
                Value::String(string) => {
                    match self
                        .categories_to_variables
                        .find_categoried_variable_with_id(string)
                    {
                        Some((category, variable)) => format!(
                            "{}.{}",
                            pymodd_class_name_of_category(category),
                            variable.enum_name
                        ),
                        _ => surround_string_with_quotes(string),
                    }
                }
                Value::Bool(boolean) => String::from(match boolean {
                    true => "True",
                    false => "False",
                }),
                Value::Number(number) => number.to_string(),
                _ => String::from("None"),
            },
            ArgumentValueIterItem::Constant(constant) => constant.to_owned(),
            ArgumentValueIterItem::Condition(operation)
            | ArgumentValueIterItem::Concatenation(operation)
            | ArgumentValueIterItem::Calculation(operation) => {
                self.build_operation_content(&operation)
            }
            ArgumentValueIterItem::ScriptKey(key) => {
                let item_with_key = self.root_directory.find_item_with_key(&key);
                if item_with_key.is_some() {
                    if let DirectoryIterItem::Script(script) = item_with_key.unwrap() {
                        // run_script action accepts Script objects, not keys
                        return format!("{}()", script.pymodd_class_name());
                    }
                }
                String::from("None")
            }
            ArgumentValueIterItem::FunctionEnd => String::from(")"),
        }
    }

    fn build_operation_content(&self, operator: &Operation) -> String {
        let (item_a, operator, item_b) = (
            ArgumentValueIterItem::from_argument(&operator.item_a),
            ArgumentValueIterItem::from_argument(&operator.operator),
            ArgumentValueIterItem::from_argument(&operator.item_b),
        );

        format!(
            "{} {} {}",
            self.build_operation_item_content(item_a),
            if let ArgumentValueIterItem::Value(operator_value) = operator {
                into_operator(operator_value.as_str().unwrap_or("")).unwrap_or("")
            } else {
                ""
            },
            self.build_operation_item_content(item_b)
        )
    }

    fn build_operation_item_content(&self, operation_item: ArgumentValueIterItem) -> String {
        match operation_item {
            // only surround conditions and calculations with parenthesis
            ArgumentValueIterItem::Condition(_) | ArgumentValueIterItem::Calculation(_) => {
                format!("({})", self.build_argument_content(operation_item))
            }
            ArgumentValueIterItem::StartOfFunction(_) => self.build_arguments_content(
                ArgumentValuesIterator::from_argument_iter_value(operation_item),
            ),
            _ => self.build_argument_content(operation_item),
        }
    }

    fn build_optional_arguments_contents(&self, action: &Action) -> Vec<String> {
        let mut optional_arguments: Vec<String> = Vec::new();
        if let Some(comment) = &action.comment {
            if !comment.is_empty() {
                optional_arguments
                    .push(format!("comment={}", surround_string_with_quotes(comment)));
            }
        }
        if action.disabled {
            optional_arguments.push(String::from("disabled=True"));
        }
        if action.ran_on_client {
            optional_arguments.push(String::from("run_on_client=True"));
        }
        optional_arguments
    }
}

fn into_operator(string: &str) -> Option<&str> {
    if ["==", "!=", "<=", "<", ">", ">=", "+", "-", "/", "*", "**"].contains(&string) {
        return Some(string);
    }
    match string.to_lowercase().as_str() {
        "and" => Some("&"),
        "or" => Some("|"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use crate::game_data::{
        actions::parse_actions,
        directory::{Directory, DirectoryItem, Script},
        variable_categories::{CategoriesToVariables, Variable},
    };

    use super::ScriptsContentBuilder;

    #[test]
    fn script_content() {
        assert_eq!(
            ScriptsContentBuilder::new(
                &CategoriesToVariables::new(HashMap::new()),
                &Directory::new("root", "null", Vec::new())
            )
            .build_script_content(&Script::new(
                "initialize",
                "WI31HDK",
                vec!["gameStart"],
                Vec::new()
            )),
            String::from(format!(
                "@script(triggers=[Trigger.GAME_START])\n\
                class Initialize():\n\
                    \tdef _build(self):\n\
                        \t\tpass\n",
            ))
        );
    }

    #[test]
    fn script_with_weird_name_content() {
        assert_eq!(
            ScriptsContentBuilder::new(
                &CategoriesToVariables::new(HashMap::new()),
                &Directory::new("root", "null", Vec::new())
            )
            .build_script_content(&Script::new(
                "【 𝚒𝚗𝚒𝚝𝚒𝚊𝚕𝚒𝚣𝚎 イ】",
                "WI31HDK",
                vec!["gameStart"],
                Vec::new()
            )),
            String::from(format!(
                "@script(triggers=[Trigger.GAME_START], name='【 𝚒𝚗𝚒𝚝𝚒𝚊𝚕𝚒𝚣𝚎 イ】')\n\
                class q():\n\
                    \tdef _build(self):\n\
                        \t\tpass\n",
            ))
        );
    }

    #[test]
    fn parse_action_with_variable_into_pymodd() {
        assert_eq!(
            ScriptsContentBuilder::new(
                &CategoriesToVariables::new(HashMap::from([(
                    "shops",
                    vec![Variable::new("OJbEQyc7is", "weapons", "WEAPONS", None)]
                )])),
                &Directory::new("root", "null", Vec::new())
            )
            .build_actions_content(&parse_actions(
                &json!([
                    {
                        "type": "openShopForPlayer",
                            "player": {
                                "function": "getOwner",
                                "entity": { "function": "getLastCastingUnit", "vars": [] },
                                "vars": []
                            },
                        "shop": "OJbEQyc7is",
                        "vars": []
                    }
                ])
                .as_array()
                .unwrap()
            )),
            "open_shop_for_player(Shops.WEAPONS, OwnerOfEntity(LastCastingUnit()))\n"
        )
    }

    #[test]
    fn parse_action_with_optional_arguments_into_pymodd() {
        assert_eq!(
            ScriptsContentBuilder::new(
                &CategoriesToVariables::new(HashMap::new()),
                &Directory::new("root", "null", Vec::new())
            )
            .build_actions_content(&parse_actions(
                &json!([
                    {
                        "type": "startUsingItem",
                        "entity": { "function": "getTriggeringItem" },
                        "comment": "hi!",
                        "runOnClient": true,
                        "disabled": true,
                    }
                ])
                .as_array()
                .unwrap()
            )),
            "use_item_continuously_until_stopped(LastTriggeringItem(), comment='hi!', disabled=True, run_on_client=True)\n"
        )
    }

    #[test]
    fn parse_action_with_only_optional_arguments_into_pymodd() {
        assert_eq!(
            ScriptsContentBuilder::new(
                &CategoriesToVariables::new(HashMap::new()),
                &Directory::new("root", "null", Vec::new())
            )
            .build_actions_content(&parse_actions(
                &json!([
                    { "type": "return", "comment": "hi!", "runOnClient": true, "disabled": false, }
                ])
                .as_array()
                .unwrap()
            )),
            "return_loop(comment='hi!', run_on_client=True)\n"
        )
    }

    #[test]
    fn parse_action_with_constant_into_pymodd() {
        assert_eq!(
            ScriptsContentBuilder::new(
                &CategoriesToVariables::new(HashMap::new()),
                &Directory::new("root", "null", Vec::new())
            )
            .build_actions_content(&parse_actions(
                &json!([
                    { "type": "updateUiTextForEveryone", "target": "top", "value": "Hello!" }
                ])
                .as_array()
                .unwrap()
            )),
            "update_ui_text_for_everyone(UiTarget.TOP, 'Hello!')\n"
        )
    }

    #[test]
    fn parse_comment_action_into_pymodd() {
        assert_eq!(
            ScriptsContentBuilder::new(
                &CategoriesToVariables::new(HashMap::new()),
                &Directory::new("root", "null", Vec::new())
            )
            .build_actions_content(&parse_actions(
                &json!([
                    { "type": "comment", "comment": "hey there", }
                ])
                .as_array()
                .unwrap()
            )),
            "comment('hey there')\n"
        );
    }

    #[test]
    fn parse_nested_calculations_into_pymodd() {
        assert_eq!(
            ScriptsContentBuilder::new(
                &CategoriesToVariables::new(HashMap::new()),
                &Directory::new("root", "null", Vec::new())
            )
                .build_actions_content(&parse_actions(
                    &json!([
                        {
                            "type": "increaseVariableByNumber",
                            "variable": null,
                            "number": {
                                "function": "calculate",
                                "items": [
                                    { "operator": "*" },
                                    { "function": "getRandomNumberBetween", "min": 0, "max": 5 },
                                    { "function": "calculate", "items": [
                                            { "operator": "+" },
                                            { "function": "getExponent", "base": { "function": "currentTimeStamp" }, "power": 2 },
                                            3
                                       ]
                                    }
                                ]
                            }
                        }
                    ])
                    .as_array()
                    .unwrap()
                )),
            "increase_variable_by_number(None, RandomNumberBetween(0, 5) * ((CurrentUnixTimeStamp() ** 2) + 3))\n"
        );
    }

    #[test]
    fn parse_nested_concatenations_into_pymodd() {
        assert_eq!(
            ScriptsContentBuilder::new(
                &CategoriesToVariables::new(HashMap::new()),
                &Directory::new("root", "null", Vec::new())
            )
                .build_actions_content(&parse_actions(
                    &json!([
                        {
                            "type": "sendChatMessage",
                            "message": {
                                "function": "concat",
                                "textA": "hi ",
                                "textB": {
                                    "function": "concat",
                                    "textA": {
                                        "function": "getPlayerId",
                                        "player": { "function": "getTriggeringPlayer" }
                                    },
                                    "textB": " player!"
                                }
                            }
                        }
                    ])
                    .as_array()
                    .unwrap()
                )),
            "send_chat_message_to_everyone('hi ' + IdOfPlayer(LastTriggeringPlayer()) + ' player!')\n"
        );
    }

    #[test]
    fn parse_nested_if_statements_into_pymodd() {
        assert_eq!(
            ScriptsContentBuilder::new(
                &CategoriesToVariables::new(HashMap::new()),
                &Directory::new("root", "null", Vec::new())
            )
            .build_actions_content(&parse_actions(
                json!([
                     {
                        "type": "condition",
                        "conditions": [
                            { "operandType": "boolean", "operator": "==" }, true, true
                        ],
                        "then": [
                            {
                                "type": "condition",
                                "conditions": [
                                    { "operandType": "boolean", "operator": "==" }, true, true
                                ],
                                "then": [
                                    {
                                        "type": "condition",
                                        "conditions": [
                                            { "operandType": "boolean", "operator": "==" }, true, true
                                        ],
                                        "then": [
                                            { "type": "sendChatMessage", "message": "hi" }
                                        ],
                                        "else": [
                                            { "type": "sendChatMessage", "message": "hi" }
                                        ]
                                    }
                                ],
                                "else": [
                                    { "type": "sendChatMessage", "message": "hi" }
                                ]
                            }
                        ],
                        "else": [
                            { "type": "sendChatMessage", "message": "hi" }
                        ]
                     }
                ])
                .as_array()
                .unwrap(),
            ))
            .as_str(),
            "if True == True:\n\
                \tif True == True:\n\
    		        \t\tif True == True:\n\
		                \t\t\tsend_chat_message_to_everyone('hi')\n\
                    \t\telse:\n\
		                \t\t\tsend_chat_message_to_everyone('hi')\n\
                \telse:\n\
		            \t\tsend_chat_message_to_everyone('hi')\n\
            else:\n\
                \tsend_chat_message_to_everyone('hi')\n"
        )
    }

    #[test]
    fn parse_nested_conditions_into_pymodd() {
        assert_eq!(
            ScriptsContentBuilder::new(
                &CategoriesToVariables::new(HashMap::new()),
                &Directory::new("root", "null", Vec::new())
            )
                .build_actions_content(&parse_actions(
                    json!([
                         {
                            "type": "condition",
                            "conditions": [
                                { "operandType": "and", "operator": "AND" },
                                [
                                    { "operandType": "boolean", "operator": "==" },
                                    { "function": "getNumberOfUnitsOfUnitType", "unitType": "oTDQ3jlcMa" },
                                    5
                                ],
                                [
                                    { "operandType": "boolean", "operator": "==" },
                                    true,
                                    true
                                ]
                            ],
                            "then": [],
                            "else": []
                         }
                    ])
                    .as_array()
                    .unwrap(),
                ))
                .as_str(),
            "if (NumberOfUnitsOfUnitType('oTDQ3jlcMa') == 5) & (True == True):\n\
                \tpass\n"
        );
    }

    #[test]
    fn parse_variable_for_loop_into_pymodd() {
        assert_eq!(
            ScriptsContentBuilder::new(
                &CategoriesToVariables::new(HashMap::from([(
                    "variables",
                    vec![Variable::new("i", "i", "I", Some("number"))]
                )])),
                &Directory::new("root", "null", Vec::new())
            )
            .build_actions_content(&parse_actions(
                json!([
                    { "type": "for", "variableName": "i", "start": 0, "stop": 5, "actions": [] }
                ])
                .as_array()
                .unwrap(),
            ))
            .as_str(),
            "for Variables.I in range(0, 5):\n\
                \tpass\n"
        );
    }

    #[test]
    fn parse_repeat_action_into_python() {
        assert_eq!(
            ScriptsContentBuilder::new(
                &CategoriesToVariables::new(HashMap::new()),
                &Directory::new("root", "null", Vec::new())
            )
            .build_actions_content(&parse_actions(
                json!([
                    { "type": "repeat", "count": 5, "actions": [] }
                ])
                .as_array()
                .unwrap(),
            ))
            .as_str(),
            "for _ in repeat(5):\n\
                \tpass\n"
        );
    }
    #[test]
    fn parse_run_script_action_into_pymodd() {
        assert_eq!(
            ScriptsContentBuilder::new(
                &CategoriesToVariables::new(HashMap::new()),
                &Directory::new(
                    "root",
                    "null",
                    vec![DirectoryItem::Directory(Directory::new(
                        "utils",
                        "n3DhW3",
                        vec![DirectoryItem::Script(Script {
                            name: String::from("spawn boss"),
                            key: String::from("If2aW3B"),
                            triggers: Vec::new(),
                            actions: Vec::new()
                        })]
                    ))]
                )
            )
            .build_actions_content(&parse_actions(
                json!([
                     {
                        "type": "runScript",
                        "scriptName": "If2aW3B"
                     }
                ])
                .as_array()
                .unwrap(),
            ))
            .as_str(),
            "run_script(SpawnBoss())\n"
        )
    }
}
