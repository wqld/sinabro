use operator::controller::Sinabro;
use kube::CustomResourceExt;

fn main() {
    print!("{}", serde_yaml::to_string(&Sinabro::crd()).unwrap())
}
