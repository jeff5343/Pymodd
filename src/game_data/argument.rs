use heck::ToSnakeCase;
use serde_json::{Map, Value};

use crate::generator::utils::to_pymodd::{PymoddStructure, FUNCTIONS_TO_PYMODD_STRUCTURE};

use super::actions::{parse_actions, Action};

const ARGS_TO_IGNORE: [&str; 4] = ["type", "function", "vars", "comment"];

/// Accepts both pymodd action and pymodd function data
pub fn parse_arguments_of_object_data(object_data: &Map<String, Value>) -> Vec<Argument> {
    let mut args = Vec::new();
    object_data
        .iter()
        .filter(|(arg_name, _)| !ARGS_TO_IGNORE.contains(&arg_name.as_str()))
        .for_each(|(arg_name, arg_data)| {
            args.extend(match arg_name.as_str() {
                // Calculate Function
                "items" => parse_arguments_of_operator_argument(arg_data),
                // Force Function
                "force" => {
                    if !arg_data
                        .as_object()
                        .unwrap_or(&Map::new())
                        .contains_key("x")
                    {
                        // if arg_data does not contain "x" key return a single force argument
                        vec![Argument::parse("force", arg_data)]
                    } else {
                        parse_arguments_of_force_object_argument(arg_data)
                    }
                }
                _ => vec![Argument::parse(arg_name, arg_data)],
            })
        });
    args
}

/// Arguments of functions Calculate and Condition are formatted differently by modd.io
fn parse_arguments_of_operator_argument(operator_argument_data: &Value) -> Vec<Argument> {
    let arguments_of_operator_argument = operator_argument_data
        .as_array()
        .unwrap_or(&Vec::new())
        .clone();
    vec![
        Argument::new(
            "item_a",
            ArgumentValue::Value(
                arguments_of_operator_argument
                    .get(1)
                    .unwrap_or(&Value::Null)
                    .clone(),
            ),
        ),
        Argument::new(
            "operator",
            ArgumentValue::Value(
                arguments_of_operator_argument
                    .get(0)
                    .unwrap_or(&Value::Null)
                    .as_object()
                    .unwrap_or(&Map::new())
                    .get("operator")
                    .unwrap_or(&Value::Null)
                    .clone(),
            ),
        ),
        Argument::new(
            "item_b",
            ArgumentValue::Value(
                arguments_of_operator_argument
                    .get(2)
                    .unwrap_or(&Value::Null)
                    .clone(),
            ),
        ),
    ]
}

fn parse_arguments_of_force_object_argument(force_argument_data: &Value) -> Vec<Argument> {
    let force_arguments_to_value = force_argument_data
        .as_object()
        .unwrap_or(&Map::new())
        .clone();
    vec![
        Argument::new(
            "x",
            ArgumentValue::Value(
                force_arguments_to_value
                    .get("x")
                    .unwrap_or(&Value::Null)
                    .clone(),
            ),
        ),
        Argument::new(
            "y",
            ArgumentValue::Value(
                force_arguments_to_value
                    .get("y")
                    .unwrap_or(&Value::Null)
                    .clone(),
            ),
        ),
    ]
}

/// Aligns the arguments to make sure they line up correctly with the structures defined in pymodd
pub fn align_arguments_with_pymodd_structure_parameters(
    mut arguments: Vec<Argument>,
    pymodd_structure_parameters: &Vec<String>,
) -> Vec<Argument> {
    let mut aligned_args: Vec<Option<Argument>> = Vec::new();
    pymodd_structure_parameters.iter().for_each(|parameter| {
        aligned_args.push(
            arguments
                .iter()
                .position(|arg| {
                    parameter.contains(&arg.name.to_snake_case())
                        || arg.name.to_snake_case().contains(parameter)
                })
                .map(|matching_arg_position| arguments.remove(matching_arg_position)),
        )
    });
    aligned_args
        .into_iter()
        .map(|value| {
            value.unwrap_or_else(|| {
                arguments
                    .pop()
                    .unwrap_or(Argument::new("null", ArgumentValue::Value(Value::Null)))
            })
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub struct Argument {
    pub name: String,
    pub value: ArgumentValue,
}

impl Argument {
    fn parse(argument_name: &str, argument_data: &Value) -> Argument {
        Argument {
            name: argument_name.to_string(),
            value: match argument_data {
                Value::Object(function_data) => {
                    match Function::name_from_data(function_data).as_str() {
                        // parse getVariable functions into variable IDs for pymodd
                        "getPlayerVariable" | "getEntityVariable" => {
                            ArgumentValue::variable_id_from_get_entity_variable_function(
                                function_data,
                            )
                        }
                        "getVariable" => {
                            ArgumentValue::variable_id_from_get_variable_function(function_data)
                        }
                        _ => ArgumentValue::Function(Function::parse(function_data)),
                    }
                }
                Value::Array(actions_data) => ArgumentValue::Actions(parse_actions(&actions_data)),
                _ => ArgumentValue::Value(argument_data.clone()),
            },
        }
    }

    fn parse_condition(condition_data: &Value) -> Argument {
        Argument::new(
            "condition",
            ArgumentValue::Function(Function::new(
                "condition",
                parse_arguments_of_operator_argument(condition_data),
            )),
        )
    }

    pub fn new(name: &str, value: ArgumentValue) -> Argument {
        Argument {
            name: name.to_string(),
            value,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ArgumentValue {
    Value(Value),
    Actions(Vec<Action>),
    Function(Function),
}

impl ArgumentValue {
    fn variable_id_from_get_entity_variable_function(
        get_entity_variable_function_data: &Map<String, Value>,
    ) -> ArgumentValue {
        ArgumentValue::Value(
            get_entity_variable_function_data
                .get("variable")
                .unwrap_or(&Value::Object(Map::new()))
                .get("key")
                .unwrap_or(&Value::Null)
                .to_owned(),
        )
    }

    fn variable_id_from_get_variable_function(
        get_variable_function_data: &Map<String, Value>,
    ) -> ArgumentValue {
        ArgumentValue::Value(
            get_variable_function_data
                .get("variableName")
                .unwrap_or(&Value::Null)
                .to_owned(),
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Function {
    name: String,
    pub args: Vec<Argument>,
}

impl Function {
    fn parse(function_data: &Map<String, Value>) -> Function {
        let name = Function::name_from_data(function_data);
        Function {
            args: align_arguments_with_pymodd_structure_parameters(
                parse_arguments_of_object_data(function_data),
                &FUNCTIONS_TO_PYMODD_STRUCTURE
                    .get(&name)
                    .unwrap_or(&PymoddStructure::default())
                    .parameters,
            ),
            name,
        }
    }

    pub fn new(name: &str, args: Vec<Argument>) -> Function {
        Function {
            name: name.to_string(),
            args,
        }
    }

    fn name_from_data(function_data: &Map<String, Value>) -> String {
        function_data
            .get("function")
            .unwrap_or(&Value::Null)
            .as_str()
            .unwrap_or("null")
            .to_string()
    }

    pub fn pymodd_class_name(&self) -> String {
        FUNCTIONS_TO_PYMODD_STRUCTURE
            .get(&self.name)
            .unwrap_or(&PymoddStructure::default())
            .name
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        game_data::argument::parse_arguments_of_object_data,
        generator::utils::to_pymodd::{ACTIONS_TO_PYMODD_STRUCTURE, FUNCTIONS_TO_PYMODD_STRUCTURE},
    };

    use super::{
        align_arguments_with_pymodd_structure_parameters, parse_arguments_of_operator_argument,
        Argument,
        ArgumentValue::{Function as Func, Value as Val},
        Function,
    };
    use serde_json::{json, Value};

    #[test]
    fn align_condition_arguments_with_pymodd() {
        assert_eq!(
            align_arguments_with_pymodd_structure_parameters(
                vec![
                    Argument::new("operator", Val(Value::String("==".to_string()))),
                    Argument::new("item_a", Val(Value::Bool(true))),
                    Argument::new("item_b", Val(Value::Bool(true))),
                ],
                &FUNCTIONS_TO_PYMODD_STRUCTURE
                    .get("condition")
                    .unwrap()
                    .parameters
            )
            .as_slice(),
            [
                Argument::new("item_a", Val(Value::Bool(true))),
                Argument::new("operator", Val(Value::String("==".to_string()))),
                Argument::new("item_b", Val(Value::Bool(true))),
            ]
        )
    }

    #[test]
    fn align_action_arguments_with_pymodd() {
        assert_eq!(
            align_arguments_with_pymodd_structure_parameters(
                vec![
                    Argument::new("variableType", Val(Value::Null)),
                    Argument::new("value", Val(Value::Null)),
                    Argument::new("not_matching", Val(Value::Null)),
                ],
                &ACTIONS_TO_PYMODD_STRUCTURE
                    .get("setPlayerVariable")
                    .unwrap()
                    .parameters
            )
            .as_slice(),
            [
                Argument::new("not_matching", Val(Value::Null)),
                Argument::new("variableType", Val(Value::Null)),
                Argument::new("value", Val(Value::Null)),
            ]
        )
    }

    #[test]
    fn parse_force_object_argument() {
        assert_eq!(
            parse_arguments_of_object_data(
                &json!({
                    "force": {
                        "x": 1,
                        "y": 1
                    }
                })
                .as_object()
                .unwrap()
            )
            .as_slice(),
            [
                Argument::new("x", Val(json!(1))),
                Argument::new("y", Val(json!(1)))
            ]
        );
    }

    #[test]
    fn parse_regular_force_argument() {
        assert_eq!(
            parse_arguments_of_object_data(
                &json!({
                    "force": 5,
                })
                .as_object()
                .unwrap()
            )
            .as_slice(),
            [Argument::new("force", Val(json!(5))),]
        );
    }

    #[test]
    fn parse_player_variable_argument() {
        assert_eq!(
            parse_arguments_of_object_data(
                &json!({
                    "variable": {
                        "function": "getPlayerVariable",
                        "variable": {
                            "text": "unit",
                            "dataType": "unit",
                            "entity": "humanPlayer",
                            "key": "OW31JD2"
                        }
                    }
                })
                .as_object()
                .unwrap()
            )
            .as_slice(),
            [Argument::new(
                "variable",
                Val(Value::String("OW31JD2".to_string()))
            )]
        );
    }

    #[test]
    fn parse_get_variable_argument() {
        assert_eq!(
            parse_arguments_of_object_data(
                &json!({
                    "value": {
                        "function": "getVariable",
                        "variableName": "AI"
                    },
                })
                .as_object()
                .unwrap()
            )
            .as_slice(),
            [Argument::new("value", Val(Value::String("AI".to_string())))]
        );
    }

    #[test]
    fn parse_calculate_function() {
        assert_eq!(
            Function::parse(
                &json!({
                    "function": "calculate",
                        "items": [
                             {
                                  "operator": "+"
                             },
                             1,
                             5
                        ]
                })
                .as_object()
                .unwrap()
            ),
            Function::new(
                "calculate",
                vec![
                    Argument::new("item_a", Val(json!(1))),
                    Argument::new("operator", Val(Value::String("+".to_string()))),
                    Argument::new("item_b", Val(json!(5))),
                ]
            )
        );
    }
}
