//! Database enum mappings for domain types.
//!
//! Maps pure domain enums to their database representations.
//! The domain layer stays clean — all DB concerns are here.

hexforge::impl_db_enum! {
    crate::domain::EntryType => "entry_type", rename_all = "lowercase" {
        Article,
        Book,
        Incollection,
        Inproceedings,
        Mastersthesis,
        Misc,
        Phdthesis,
        Proceedings,
        Techreport,
        Unpublished,
        Unknown = "UNKNOWN",
    }
}

hexforge::impl_db_enum! {
    crate::domain::PubState => "pubstate", rename_all = "lowercase" {
        Unpub,
        Forthcoming,
        Inwork,
        Submitted,
        Published,
    }
}

hexforge::impl_db_enum! {
    crate::domain::LangId => "langid", rename_all = "lowercase" {
        Catalan,
        Czech,
        Danish,
        Dutch,
        English,
        French,
        Greek,
        Italian,
        Latin,
        Lithuanian,
        Ngerman,
        Polish,
        Portuguese,
        Romanian,
        Russian,
        Slovak,
        Spanish,
        Swedish,
        Unknown,
    }
}

hexforge::impl_db_enum! {
    crate::domain::Epoch => "epoch", rename_all = "kebab-case" {
        AncientPhilosophy,
        AncientScientists,
        AustrianPhilosophy,
        BritishIdealism,
        Classics,
        Contemporaries,
        ContemporaryScientists,
        ContinentalPhilosophy,
        CriticalTheory,
        Cynics,
        Enlightenment,
        Existentialism,
        ExoticPhilosophy,
        GermanIdealism,
        GermanRationalism,
        GestaltPsychology,
        Hermeneutics,
        IslamicPhilosophy,
        Mathematicians,
        MedievalPhilosophy,
        ModernPhilosophy,
        ModernScientists,
        NeoKantianism,
        Neoplatonism,
        NewRealism,
        OrdinaryLanguagePhilosophy,
        Phenomenology,
        PolishLogic,
        Pragmatism,
        Presocratics,
        Renaissance,
        Stoics,
        Theologians,
        ViennaCircle,
    }
}

hexforge::impl_db_enum! {
    crate::domain::AuthorRole => "author_role", rename_all = "lowercase" {
        Author,
        Editor,
        Guesteditor,
    }
}

hexforge::impl_db_enum! {
    crate::domain::RefType => "ref_type", rename_all = "snake_case" {
        FurtherRef,
        DependsOn,
    }
}

hexforge::impl_db_enum! {
    crate::domain::PermissionLevel => "permission_level", rename_all = "lowercase" {
        Public,
        Read,
        Write,
        Admin,
    }
}
