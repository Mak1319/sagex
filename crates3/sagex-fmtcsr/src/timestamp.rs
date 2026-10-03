use chrono::{DateTime, Utc};
use rkyv::{
    rancor::{Fallible, OptionExt, Source},
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, Archived, Deserialize, Place, Resolver, Serialize,
};

type Pair = (i64, u32);

pub struct SecsNanos;

impl ArchiveWith<DateTime<Utc>> for SecsNanos {
    type Archived = Archived<Pair>;
    type Resolver = Resolver<Pair>;

    fn resolve_with(
        field: &DateTime<Utc>,
        resolver: Self::Resolver,
        out: Place<Self::Archived>,
    ) {
        (field.timestamp(), field.timestamp_subsec_nanos()).resolve(resolver, out);
    }
}

impl<S> SerializeWith<DateTime<Utc>, S> for SecsNanos
where
    S: Fallible + ?Sized,
    Pair: Serialize<S>,
{
    fn serialize_with(
        field: &DateTime<Utc>,
        serializer: &mut S,
    ) -> Result<Self::Resolver, S::Error> {
        (field.timestamp(), field.timestamp_subsec_nanos()).serialize(serializer)
    }
}

impl<D> DeserializeWith<Archived<Pair>, DateTime<Utc>, D> for SecsNanos
where
    D: Fallible + ?Sized,
    D::Error: Source,
    Archived<Pair>: Deserialize<Pair, D>,
{
    fn deserialize_with(
        field: &Archived<Pair>,
        deserializer: &mut D,
    ) -> Result<DateTime<Utc>, D::Error> {
        let (secs, nanos): Pair = Deserialize::deserialize(field, deserializer)?;
        DateTime::from_timestamp(secs, nanos).into_trace("invalid timestamp")
    }
}
