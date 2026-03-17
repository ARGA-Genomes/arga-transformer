use tracing::{info, instrument};

use crate::dataset::{Dataset, Model};
use crate::errors::TransformError;
use crate::rdf::{self, ProjectMemberField};
use crate::resolver::{ResolvedRecords, Resolver};


#[derive(Debug, Default, serde::Serialize)]
pub struct ProjectMember {
    pub entity_id: String,
    pub project_id: Option<String>,
    pub name: Option<String>,
    pub orcid: Option<String>,
    pub organisation: Option<String>,
}


#[instrument(skip_all)]
pub fn get_all(dataset: &Dataset) -> Result<Vec<ProjectMember>, TransformError> {
    let resolver = Resolver::new(dataset);

    let schemas = dataset.scope(&[Model::ProjectMember]);
    let schemas: Vec<&iref::Iri> = schemas.iter().map(|s| s.as_iri()).collect();

    info!("Resolving data");
    let data: ResolvedRecords<ProjectMemberField> = resolver.resolve(rdf::ProjectMember::ALL, &schemas)?;


    let mut members = Vec::new();

    for (_idx, fields) in data {
        let mut member = ProjectMember::default();

        for field in fields {
            match field {
                ProjectMemberField::EntityId(val) => member.entity_id = val,
                ProjectMemberField::ProjectId(val) => member.project_id = Some(val),
                ProjectMemberField::Name(val) => member.name = Some(val),
                ProjectMemberField::Orcid(val) => member.orcid = Some(val),
                ProjectMemberField::Organisation(val) => member.organisation = Some(val),
            }
        }

        members.push(member);
    }

    Ok(members)
}
