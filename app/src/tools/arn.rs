//! AWS ARN Parser: splits an Amazon Resource Name into partition, service,
//! region, account and resource. Finds every ARN in the input, so a whole
//! IAM policy or log line can be pasted.
use regex::Regex;
use std::sync::LazyLock;
use super::{Category, TextTool, Tool};
use crate::ui::icon;

/// Example for the "Sample" button: a Lambda function alias.
const SAMPLE: &str = "arn:aws:lambda:ca-central-1:123456789012:function:order-processor:PROD";

/// Shown when an ARN does not have its six parts.
const FORMAT: &str = "An ARN has six parts: arn:partition:service:region:account-id:resource";

/// `arn:` up to the first character that cannot be part of an ARN in JSON,
/// YAML or a log line (whitespace, quotes, commas, closing brackets).
static ARN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"arn:[^\s"'`,\]\)\}>]+"#).expect("valid pattern"));

/// Names of the partitions, by ARN prefix.
const PARTITIONS: [(&str, &str); 7] = [
    ("aws", "AWS (standard regions)"), ("aws-cn", "AWS China"), ("aws-us-gov", "AWS GovCloud (US)"),
    ("aws-iso", "AWS ISO"), ("aws-iso-b", "AWS ISOB"), ("aws-iso-e", "AWS ISOE"), ("aws-iso-f", "AWS ISOF"),
];

/// Names of common services, by their ARN code.
const SERVICES: [(&str, &str); 46] = [
    ("acm", "AWS Certificate Manager"), ("apigateway", "Amazon API Gateway"), ("appsync", "AWS AppSync"),
    ("athena", "Amazon Athena"), ("autoscaling", "Amazon EC2 Auto Scaling"), ("backup", "AWS Backup"),
    ("bedrock", "Amazon Bedrock"), ("cloudformation", "AWS CloudFormation"), ("cloudfront", "Amazon CloudFront"),
    ("cloudwatch", "Amazon CloudWatch"), ("codebuild", "AWS CodeBuild"), ("codepipeline", "AWS CodePipeline"),
    ("cognito-idp", "Amazon Cognito user pools"), ("dynamodb", "Amazon DynamoDB"), ("ec2", "Amazon EC2"),
    ("ecr", "Amazon ECR"), ("ecs", "Amazon ECS"), ("eks", "Amazon EKS"), ("elasticache", "Amazon ElastiCache"),
    ("elasticfilesystem", "Amazon EFS"), ("elasticloadbalancing", "Elastic Load Balancing"), ("es", "Amazon OpenSearch Service"),
    ("events", "Amazon EventBridge"), ("execute-api", "Amazon API Gateway (invoke)"), ("firehose", "Amazon Data Firehose"),
    ("glue", "AWS Glue"), ("iam", "AWS Identity and Access Management"), ("kinesis", "Amazon Kinesis Data Streams"),
    ("kms", "AWS Key Management Service"), ("lambda", "AWS Lambda"), ("logs", "Amazon CloudWatch Logs"),
    ("organizations", "AWS Organizations"), ("rds", "Amazon RDS"), ("redshift", "Amazon Redshift"),
    ("route53", "Amazon Route 53"), ("s3", "Amazon S3"), ("sagemaker", "Amazon SageMaker"),
    ("scheduler", "Amazon EventBridge Scheduler"), ("secretsmanager", "AWS Secrets Manager"), ("ses", "Amazon SES"),
    ("sns", "Amazon SNS"), ("sqs", "Amazon SQS"), ("ssm", "AWS Systems Manager"), ("states", "AWS Step Functions"),
    ("sts", "AWS Security Token Service"), ("wafv2", "AWS WAF"),
];

/// Names of the regions, by code.
const REGIONS: [(&str, &str); 37] = [
    ("us-east-1", "US East (N. Virginia)"), ("us-east-2", "US East (Ohio)"), ("us-west-1", "US West (N. California)"),
    ("us-west-2", "US West (Oregon)"), ("af-south-1", "Africa (Cape Town)"), ("ap-east-1", "Asia Pacific (Hong Kong)"),
    ("ap-east-2", "Asia Pacific (Taipei)"), ("ap-south-1", "Asia Pacific (Mumbai)"), ("ap-south-2", "Asia Pacific (Hyderabad)"),
    ("ap-southeast-1", "Asia Pacific (Singapore)"), ("ap-southeast-2", "Asia Pacific (Sydney)"), ("ap-southeast-3", "Asia Pacific (Jakarta)"),
    ("ap-southeast-4", "Asia Pacific (Melbourne)"), ("ap-southeast-5", "Asia Pacific (Malaysia)"), ("ap-southeast-7", "Asia Pacific (Thailand)"),
    ("ap-northeast-1", "Asia Pacific (Tokyo)"), ("ap-northeast-2", "Asia Pacific (Seoul)"), ("ap-northeast-3", "Asia Pacific (Osaka)"),
    ("ca-central-1", "Canada (Central)"), ("ca-west-1", "Canada West (Calgary)"), ("eu-central-1", "Europe (Frankfurt)"),
    ("eu-central-2", "Europe (Zurich)"), ("eu-west-1", "Europe (Ireland)"), ("eu-west-2", "Europe (London)"),
    ("eu-west-3", "Europe (Paris)"), ("eu-south-1", "Europe (Milan)"), ("eu-south-2", "Europe (Spain)"),
    ("eu-north-1", "Europe (Stockholm)"), ("il-central-1", "Israel (Tel Aviv)"), ("me-south-1", "Middle East (Bahrain)"),
    ("me-central-1", "Middle East (UAE)"), ("mx-central-1", "Mexico (Central)"), ("sa-east-1", "South America (São Paulo)"),
    ("us-gov-east-1", "AWS GovCloud (US-East)"), ("us-gov-west-1", "AWS GovCloud (US-West)"),
    ("cn-north-1", "China (Beijing)"), ("cn-northwest-1", "China (Ningxia)"),
];

/// The AWS ARN Parser tool.
pub fn arn() -> Box<dyn Tool> {
    Box::new(TextTool::new("AWS ARN Parser", icon::CLOUD, Category::Security, vec![("Parse", parse_all)])
        .describe("Splits an ARN into partition, service, region, account and resource. Paste one, or text that contains several (a policy, a log line…).")
        .sample(SAMPLE).key_value_output())
}

/// The name for `code` in a `(code, name)` table, if it is listed.
fn name_of(table: &[(&str, &'static str)], code: &str) -> Option<&'static str> {
    table.iter().find(|(c, _)| *c == code).map(|(_, name)| *name)
}

/// The report of every distinct ARN in `input`: a flat report for one, a
/// section headed by the ARN for each when there are several.
fn parse_all(input: &str) -> Result<String, String> {
    let mut arns: Vec<&str> = Vec::new();
    for found in ARN.find_iter(input) {
        // A trailing period or semicolon ends a sentence or a statement, not the ARN.
        let arn = found.as_str().trim_end_matches(['.', ';']);
        if !arns.contains(&arn) { arns.push(arn); }
    }
    match arns.as_slice() {
        [] => Err("No ARN found: an ARN starts with \"arn:\", e.g. arn:aws:s3:::my-bucket".into()),
        [one] => parse(one),
        many => Ok(many.iter().map(|arn| {
            let lines = parse(arn).unwrap_or_else(|e| format!("Error: {e}"));
            format!("{arn}:\n{}", lines.lines().map(|l| format!("  {l}")).collect::<Vec<_>>().join("\n"))
        }).collect::<Vec<_>>().join("\n\n")),
    }
}

/// The `Key: value` report of one ARN.
fn parse(arn: &str) -> Result<String, String> {
    let parts: Vec<&str> = arn.splitn(6, ':').collect();
    let [_, partition, service, region, account, resource] = parts.as_slice() else { return Err(FORMAT.into()) };
    if partition.is_empty() || service.is_empty() || resource.is_empty() { return Err(FORMAT.into()); }
    let mut lines = vec![format!("Partition: {partition}")];
    if let Some(name) = name_of(&PARTITIONS, partition) { lines.push(format!("Partition name: {name}")); }
    lines.push(format!("Service: {service}"));
    if let Some(name) = name_of(&SERVICES, service) { lines.push(format!("Service name: {name}")); }
    if region.is_empty() {
        lines.push("Region: none (global)".into());
    } else {
        lines.push(format!("Region: {region}"));
        if let Some(name) = name_of(&REGIONS, region) { lines.push(format!("Region name: {name}")); }
    }
    match *account {
        "" => lines.push("Account: none".into()),
        // AWS managed policies belong to the "aws" account.
        "aws" => lines.push("Account: aws (managed by AWS)".into()),
        id => {
            lines.push(format!("Account: {id}"));
            if !(id.len() == 12 && id.bytes().all(|b| b.is_ascii_digit())) && !id.contains(['*', '?']) {
                lines.push("Warning: account IDs have 12 digits".into());
            }
        }
    }
    lines.push(String::new());
    lines.push(format!("Resource: {resource}"));
    if *service == "s3" && region.is_empty() && account.is_empty() {
        // arn:aws:s3:::bucket/key: the bucket name comes first.
        let (bucket, key) = resource.split_once('/').unwrap_or((resource, ""));
        lines.push(format!("Bucket: {bucket}"));
        if !key.is_empty() { lines.push(format!("Object key: {key}")); }
    } else if let Some(at) = resource.find(['/', ':']) {
        // `type/id` or `type:id`; the ID may itself contain `/` or `:`.
        lines.push(format!("Resource type: {}", &resource[..at]));
        lines.push(format!("Resource ID: {}", &resource[at + 1..]));
    }
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_an_arn() {
        let out = parse_all(SAMPLE).unwrap();
        for line in ["Partition: aws", "Service: lambda", "Service name: AWS Lambda", "Region: ca-central-1", "Region name: Canada (Central)",
            "Account: 123456789012", "Resource: function:order-processor:PROD", "Resource type: function", "Resource ID: order-processor:PROD"] {
            assert!(out.lines().any(|l| l == line), "{line} missing in\n{out}");
        }
        assert!(!out.contains("Warning"), "{out}");
    }

    #[test]
    fn handles_global_and_special_resources() {
        let s3 = parse_all("arn:aws:s3:::my-bucket/logs/2026/app.log").unwrap();
        assert!(s3.contains("Region: none (global)\nAccount: none"), "{s3}");
        assert!(s3.contains("Bucket: my-bucket\nObject key: logs/2026/app.log"), "{s3}");
        let policy = parse_all("arn:aws:iam::aws:policy/ReadOnlyAccess").unwrap();
        assert!(policy.contains("Account: aws (managed by AWS)"), "{policy}");
        assert!(policy.contains("Resource type: policy\nResource ID: ReadOnlyAccess"), "{policy}");
        // A plain name has no type.
        let topic = parse_all("arn:aws-cn:sns:cn-north-1:123456789012:alerts").unwrap();
        assert!(topic.contains("Partition name: AWS China") && !topic.contains("Resource type"), "{topic}");
        assert!(parse_all("arn:aws:iam::1234:role/x").unwrap().contains("Warning: account IDs have 12 digits"));
    }

    #[test]
    fn finds_every_arn_in_a_text() {
        let policy = r#"{"Resource": ["arn:aws:s3:::a-bucket", "arn:aws:s3:::a-bucket/*", "arn:aws:s3:::a-bucket"]}"#;
        let out = parse_all(policy).unwrap();
        // Two distinct ARNs, each in its own section; the duplicate is left out.
        assert!(out.starts_with("arn:aws:s3:::a-bucket:\n  Partition: aws"), "{out}");
        assert!(out.contains("\n\narn:aws:s3:::a-bucket/*:\n"), "{out}");
        assert_eq!(out.matches("Bucket: a-bucket").count(), 2, "{out}");
        // The period ends the sentence, not the queue name.
        assert!(parse_all("see arn:aws:sqs:us-east-1:123456789012:jobs.").unwrap().ends_with("Resource: jobs"));
        assert!(parse_all("no ARN here").is_err());
        assert!(parse_all("arn:aws:s3").is_err());
    }
}
