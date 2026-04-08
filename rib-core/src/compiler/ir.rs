// Copyright 2024-2025 Golem Cloud
//
// Licensed under the Golem Source License v1.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://license.golem.cloud/LICENSE
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::wit_type::WitType;
use crate::ValueAndType;
use crate::{WitTypeWithUnit, ComponentDependencyKey, ParsedFunctionSite, VariableId};
use serde::{Deserialize, Serialize};

// To create any type, example, CreateOption, you have to feed a fully formed WitType
#[derive(Debug, Clone, PartialEq)]
pub enum RibIR {
    PushLit(ValueAndType),
    AssignVar(VariableId),
    LoadVar(VariableId),
    CreateAndPushRecord(WitType),
    UpdateRecord(String),
    PushList(WitType, usize),
    PushTuple(WitType, usize),
    PushSome(WitType),
    PushNone(Option<WitType>), // In certain cases, we don't need the type info
    PushOkResult(WitType),
    PushErrResult(WitType),
    PushFlag(ValueAndType), // More or less like a literal, compiler can form the value directly
    SelectField(String),
    SelectIndex(usize), // Kept for backward compatibility. Cannot read old SelectIndex(usize) as a SelectIndexV1
    SelectIndexV1,
    EqualTo,
    GreaterThan,
    And,
    Or,
    LessThan,
    GreaterThanOrEqualTo,
    LessThanOrEqualTo,
    IsEmpty,
    JumpIfFalse(InstructionId),
    Jump(InstructionId),
    Label(InstructionId),
    Deconstruct,
    CreateFunctionName(ParsedFunctionSite, FunctionReferenceType),
    InvokeFunction(
        ComponentDependencyKey,
        InstanceVariable,
        usize,
        WitTypeWithUnit,
    ),
    PushVariant(String, WitType), // There is no arg size since the type of each variant case is only 1 from beginning
    PushEnum(String, WitType),
    Throw(String),
    GetTag,
    Concat(usize),
    Plus(WitType),
    Minus(WitType),
    Divide(WitType),
    Multiply(WitType),
    Negate,
    ToIterator,
    CreateSink(WitType),
    AdvanceIterator,
    PushToSink,
    SinkToList,
    Length,
    GenerateWorkerName(Option<VariableId>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum InstanceVariable {
    WitResource(VariableId),
    WitWorker(VariableId),
}

impl RibIR {
    pub fn get_instruction_id(&self) -> Option<InstructionId> {
        match self {
            RibIR::Label(id) => Some(id.clone()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FunctionReferenceType {
    Function { function: String },
    RawResourceConstructor { resource: String },
    RawResourceDrop { resource: String },
    RawResourceMethod { resource: String, method: String },
    RawResourceStaticMethod { resource: String, method: String },
}

// Every instruction can have a unique ID, and the compiler
// can assign this and label the start and end of byte code blocks.
// This is more efficient than assigning index to every instruction and incrementing it
// as we care about it only if we need to jump through instructions.
// Jumping to an ID is simply draining the stack until we find a Label instruction with the same ID.
#[derive(Debug, Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub struct InstructionId {
    pub index: usize,
}

impl InstructionId {
    pub fn new(index: usize) -> Self {
        InstructionId { index }
    }

    pub fn init() -> Self {
        InstructionId { index: 0 }
    }

    pub fn increment(&self) -> InstructionId {
        InstructionId {
            index: self.index + 1,
        }
    }

    pub fn increment_mut(&mut self) -> InstructionId {
        self.index += 1;
        self.clone()
    }
}
