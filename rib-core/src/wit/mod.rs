// Copyright 2024-2025 Golem Cloud
//
// Licensed under the Golem Source License v1.0 (the "License");
// you may not use this file except in compliance with the License.
//
//     http://license.golem.cloud/LICENSE
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// `wit` is the new public home for types derived from WIT / component metadata.
// To avoid a huge file move churn, we keep the implementation in the existing
// `analysis/model.rs` for now and re-export it from here.
#[path = "../analysis/model.rs"]
mod model;

pub use model::*;

