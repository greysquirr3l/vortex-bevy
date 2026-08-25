use std::{error::Error, str::FromStr};

use vortex_core::domain::{
    ArtifactHash, ArtifactKind, ArtifactRecord, ClassificationKind, ConversionAggregate,
    ConversionClassification, ConversionContext, ConversionState, DomainError, ExportProfileName,
    ProjectRoot, Remediation, SourceAssetPath, SourceIdentity, UnrealObjectPath,
};

fn source_identity() -> Result<SourceIdentity, Box<dyn Error>> {
    Ok(SourceIdentity::Unreal {
        project_root: ProjectRoot::from_str("FixtureProject")?,
        asset_path: SourceAssetPath::from_str("Content/Props/Chair.uasset")?,
        object_path: UnrealObjectPath::from_str("/Game/Props/Chair.Chair")?,
    })
}

fn context() -> Result<ConversionContext, Box<dyn Error>> {
    Ok(ConversionContext::new(
        ExportProfileName::from_str("bevy-default")?,
        ArtifactHash::from_bytes(b"bevy-default-profile"),
        "0.1.0",
    )?)
}

fn remediation() -> Result<Remediation, Box<dyn Error>> {
    Ok(Remediation::new("Use an approved source adapter.")?)
}

#[test]
fn identity_parsing_rejects_unsafe_paths() {
    assert!(ProjectRoot::from_str("../escape").is_err());
    assert!(SourceAssetPath::from_str("Content//Chair.uasset").is_err());
    assert!(UnrealObjectPath::from_str("Game/Props/Chair").is_err());
    assert!(ron::de::from_str::<SourceAssetPath>("\"../escape\"").is_err());
}

#[test]
fn conversion_id_is_deterministic() -> Result<(), Box<dyn Error>> {
    let first = ConversionAggregate::begin(source_identity()?, context()?);
    let second = ConversionAggregate::begin(source_identity()?, context()?);

    assert_eq!(first.id(), second.id());
    Ok(())
}

#[test]
fn every_classification_reaches_a_terminal_state() -> Result<(), Box<dyn Error>> {
    let classifications = [
        ClassificationKind::Converted,
        ClassificationKind::Baked,
        ClassificationKind::RequiresAdapter,
        ClassificationKind::Unsupported,
        ClassificationKind::Rejected,
    ];

    for kind in classifications {
        let mut aggregate = ConversionAggregate::begin(source_identity()?, context()?);
        if kind == ClassificationKind::Converted {
            aggregate.record_artifact(ArtifactRecord::new(
                ArtifactKind::Glb,
                ArtifactHash::from_bytes(b"fixture-glb"),
                "fixture.glb",
            )?)?;
            aggregate.record_artifact(ArtifactRecord::new(
                ArtifactKind::Manifest,
                ArtifactHash::from_bytes(b"fixture-manifest"),
                "scene.manifest.ron",
            )?)?;
        }
        aggregate.classify(ConversionClassification::new(
            kind,
            "fixture classification",
            remediation()?,
        )?)?;
        let aggregate = aggregate.finalize()?;
        assert_eq!(aggregate.state(), &ConversionState::Finalized);
    }

    Ok(())
}

#[test]
fn duplicate_artifact_and_illegal_transition_are_rejected() -> Result<(), Box<dyn Error>> {
    let mut aggregate = ConversionAggregate::begin(source_identity()?, context()?);
    let artifact = ArtifactRecord::new(
        ArtifactKind::Report,
        ArtifactHash::from_bytes(b"fixture-report"),
        "conversion-report.ron",
    )?;
    aggregate.record_artifact(artifact.clone())?;
    let duplicate = aggregate.record_artifact(artifact);
    assert!(matches!(
        duplicate,
        Err(DomainError::DuplicateArtifact { kind: ArtifactKind::Report })
    ));

    aggregate.classify(ConversionClassification::new(
        ClassificationKind::Rejected,
        "fixture rejected",
        remediation()?,
    )?)?;
    let after_classification = aggregate.record_artifact(ArtifactRecord::new(
        ArtifactKind::Texture,
        ArtifactHash::from_bytes(b"fixture-texture"),
        "texture.png",
    )?);
    assert!(matches!(
        after_classification,
        Err(DomainError::IllegalTransition { current: ConversionState::Classified, .. })
    ));

    Ok(())
}
